use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::{sync::RwLock, task::JoinSet, time};
use tokio_util::sync::CancellationToken;
use tower_lsp::{Client, LanguageServer, LspService, jsonrpc, lsp_types};

use crate::{
    lsp_decoration as decoration, lsp_progress as progress,
    lsp_workspace::{Analyzer, AnalyzerEvent},
    models::{Crate, Loc},
    range_ops, text_conversion,
};

/// Commands supported by workspace/executeCommand
pub const CMD_TOGGLE_OWNERSHIP: &str = "ferrous-owl.toggleOwnership";
pub const CMD_ENABLE_OWNERSHIP: &str = "ferrous-owl.enableOwnership";
pub const CMD_DISABLE_OWNERSHIP: &str = "ferrous-owl.disableOwnership";
pub const CMD_ANALYZE: &str = "ferrous-owl.analyze";

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeRequest {}
#[derive(serde::Serialize, Clone, Debug)]
pub struct AnalyzeResponse {}

/// Tracks whether ownership diagnostics are enabled for each document
#[derive(Default, Clone)]
struct OwnershipState {
    /// Map from file path to (enabled, `cursor_position`)
    enabled_files: HashMap<PathBuf, (bool, Option<lsp_types::Position>)>,
}

/// `FerrousOwl` LSP server backend
pub struct Backend {
    client: Client,
    analyzers: Arc<RwLock<Vec<Analyzer>>>,
    status: Arc<RwLock<progress::AnalysisStatus>>,
    analyzed: Arc<RwLock<Option<Crate>>>,
    processes: Arc<RwLock<JoinSet<()>>>,
    process_tokens: Arc<RwLock<BTreeMap<usize, CancellationToken>>>,
    work_done_progress: Arc<RwLock<bool>>,
    /// Per-document state for ownership diagnostics display
    ownership_state: Arc<RwLock<OwnershipState>>,
}

impl Backend {
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            analyzers: Arc::new(RwLock::new(Vec::new())),
            analyzed: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(progress::AnalysisStatus::Finished)),
            processes: Arc::new(RwLock::new(JoinSet::new())),
            process_tokens: Arc::new(RwLock::new(BTreeMap::new())),
            work_done_progress: Arc::new(RwLock::new(false)),
            ownership_state: Arc::new(RwLock::new(OwnershipState::default())),
        }
    }

    async fn add_analyze_target(&self, path: &Path) -> bool {
        if let Ok(new_analyzer) = Analyzer::new(&path).await {
            let mut analyzers = self.analyzers.write().await;
            for analyzer in &*analyzers {
                if analyzer.target_path() == new_analyzer.target_path() {
                    return true;
                }
            }
            analyzers.push(new_analyzer);
            true
        } else {
            false
        }
    }

    pub async fn analyze(&self, _params: AnalyzeRequest) -> jsonrpc::Result<AnalyzeResponse> {
        log::info!("ferrous-owl/analyze request received");
        self.do_analyze().await;
        Ok(AnalyzeResponse {})
    }
    async fn do_analyze(&self) {
        self.shutdown_subprocesses().await;
        // Use all_targets=true by default to include test code
        self.analyze_with_options(true, false).await;
    }

    async fn analyze_with_options(&self, all_targets: bool, all_features: bool) {
        log::info!("wait 100ms for rust-analyzer");
        time::sleep(time::Duration::from_millis(100)).await;

        log::info!("stop running analysis processes");
        self.shutdown_subprocesses().await;

        log::info!("start analysis");
        {
            *self.status.write().await = progress::AnalysisStatus::Analyzing;
        }
        let analyzers = { self.analyzers.read().await.clone() };

        log::info!("analyze {} packages...", analyzers.len());
        for analyzer in analyzers {
            let analyzed = self.analyzed.clone();
            let client = self.client.clone();
            let work_done_progress = self.work_done_progress.clone();
            let cancellation_token = CancellationToken::new();

            let cancellation_token_key = {
                let token = cancellation_token.clone();
                let mut tokens = self.process_tokens.write().await;
                let key = tokens
                    .last_entry()
                    .map(|v| *v.key())
                    .map_or(1, |key| key + 1);
                tokens.insert(key, token);
                key
            };

            let process_tokens = self.process_tokens.clone();
            self.processes.write().await.spawn(async move {
                #[allow(
                    clippy::if_then_some_else_none,
                    reason = "cannot use bool::then with async await"
                )]
                let progress_token = if *work_done_progress.read().await {
                    Some(progress::ProgressToken::begin(client.clone(), None::<&str>).await)
                } else {
                    None
                };

                let mut iter = analyzer.analyze(all_targets, all_features).await;
                let mut analyzed_package_count = 0;
                while let Some(event) = tokio::select! {
                    () = cancellation_token.cancelled() => None,
                    event = iter.next_event() => event,
                } {
                    match event {
                        AnalyzerEvent::CrateChecked {
                            package,
                            package_count,
                        } => {
                            analyzed_package_count += 1;
                            if let Some(token) = &progress_token {
                                let percentage =
                                    (analyzed_package_count * 100 / package_count).min(100);
                                #[allow(
                                    clippy::cast_possible_truncation,
                                    reason = "percentage is 0-100"
                                )]
                                let percentage_u32 = percentage as u32;
                                token
                                    .report(
                                        Some(format!("{package} analyzed")),
                                        Some(percentage_u32),
                                    )
                                    .await;
                            }
                        }
                        AnalyzerEvent::Analyzed(ws) => {
                            let write = &mut *analyzed.write().await;
                            for krate in ws.0.into_values() {
                                if let Some(write) = write {
                                    write.merge(krate);
                                } else {
                                    *write = Some(krate);
                                }
                            }
                        }
                    }
                }
                // remove cancellation token from list
                process_tokens.write().await.remove(&cancellation_token_key);

                if let Some(progress_token) = progress_token {
                    progress_token.finish().await;
                }
            });
        }

        let processes = self.processes.clone();
        let status = self.status.clone();
        let analyzed = self.analyzed.clone();
        tokio::spawn(async move {
            while { processes.write().await.join_next().await }.is_some() {}
            let mut status = status.write().await;
            let analyzed = analyzed.write().await;
            if *status != progress::AnalysisStatus::Error {
                if analyzed.as_ref().map_or(0, |v| v.0.len()) == 0 {
                    *status = progress::AnalysisStatus::Error;
                } else {
                    *status = progress::AnalysisStatus::Finished;
                }
            }
        });
    }

    async fn decos(
        &self,
        filepath: &Path,
        position: Loc,
    ) -> Result<Vec<decoration::Deco>, progress::AnalysisStatus> {
        let mut selected = decoration::SelectLocal::new(position);
        let mut error = progress::AnalysisStatus::Error;
        if let Some(analyzed) = &*self.analyzed.read().await {
            log::debug!(
                "Analysis data available, {} files analyzed",
                analyzed.0.len()
            );
            let mut found_file = false;
            for (filename, file) in &analyzed.0 {
                if filepath == PathBuf::from(filename) {
                    found_file = true;
                    log::debug!("Found file {filename}, {} items", file.items.len());
                    if !file.items.is_empty() {
                        error = progress::AnalysisStatus::Finished;
                    }
                    for item in &file.items {
                        range_ops::mir_visit(item, &mut selected);
                    }
                }
            }
            if !found_file {
                log::debug!(
                    "File {} not found in analysis results. Available files: {:?}",
                    filepath.display(),
                    analyzed.0.keys().collect::<Vec<_>>()
                );
            }

            log::debug!("Selected local: {:?}", selected.selected());
            let mut calc = decoration::CalcDecos::new(selected.selected().iter().copied());
            for (filename, file) in &analyzed.0 {
                if filepath == PathBuf::from(filename) {
                    for item in &file.items {
                        range_ops::mir_visit(item, &mut calc);
                    }
                }
            }
            calc.handle_overlapping();
            let decos = calc.decorations();
            log::debug!("Calculated {} decorations", decos.len());
            if decos.is_empty() {
                Err(error)
            } else {
                Ok(decos)
            }
        } else {
            log::debug!("No analysis data available yet");
            Err(error)
        }
    }

    pub async fn cursor(
        &self,
        params: decoration::CursorRequest,
    ) -> jsonrpc::Result<decoration::Decorations> {
        let is_analyzed = self.analyzed.read().await.is_some();
        let status = *self.status.read().await;
        if let Some(path) = params.path()
            && let Ok(text) = fs::read_to_string(&path)
        {
            let position = params.position();
            let pos = Loc::from(text_conversion::line_char_to_index(
                &text,
                position.line,
                position.character,
            ));
            let (decos, status) = match self.decos(&path, pos).await {
                Ok(v) => (v, status),
                Err(e) => (
                    Vec::new(),
                    if status == progress::AnalysisStatus::Finished {
                        e
                    } else {
                        status
                    },
                ),
            };
            let items = decos.into_iter().map(|v| v.to_lsp_range(&text)).collect();
            return Ok(decoration::Decorations {
                is_analyzed,
                status,
                path: Some(path),
                items,
            });
        }
        Ok(decoration::Decorations {
            is_analyzed,
            status,
            path: None,
            items: Vec::new(),
        })
    }

    /// Publish ownership decorations as standard LSP diagnostics for a file
    async fn publish_ownership_diagnostics(&self, path: &Path, position: lsp_types::Position) {
        log::debug!(
            "publish_ownership_diagnostics called for {} at {position:?}",
            path.display()
        );
        if let Ok(text) = fs::read_to_string(path) {
            let pos = Loc::from(text_conversion::line_char_to_index(
                &text,
                position.line,
                position.character,
            ));

            let diagnostics = match self.decos(path, pos).await {
                Ok(decos) => {
                    log::debug!("Got {} decorations", decos.len());
                    decos
                        .into_iter()
                        .filter(decoration::Deco::should_show_as_diagnostic)
                        .map(|d| d.to_lsp_range(&text).to_diagnostic())
                        .collect()
                }
                Err(e) => {
                    log::debug!("No decorations, status: {e:?}");
                    Vec::new()
                }
            };

            log::debug!("Publishing {} diagnostics", diagnostics.len());
            let uri = lsp_types::Url::from_file_path(path).unwrap();
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        } else {
            log::error!("Failed to read file {}", path.display());
        }
    }

    /// Clear ownership diagnostics for a file
    async fn clear_ownership_diagnostics(&self, path: &Path) {
        if let Ok(uri) = lsp_types::Url::from_file_path(path) {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    /// Handle workspace/executeCommand for ownership visualization commands
    pub async fn handle_execute_command(
        &self,
        params: lsp_types::ExecuteCommandParams,
    ) -> jsonrpc::Result<Option<serde_json::Value>> {
        log::debug!(
            "executeCommand received: {} {:?}",
            params.command,
            params.arguments
        );

        match params.command.as_str() {
            CMD_TOGGLE_OWNERSHIP => {
                log::debug!("Processing toggleOwnership command");
                // Arguments: [document_uri, line, character]
                if let Some(args) = Self::parse_position_args(&params.arguments) {
                    let (path, position) = args;
                    log::debug!(
                        "Parsed args: path={}, position={position:?}",
                        path.display()
                    );
                    let mut state = self.ownership_state.write().await;
                    let entry = state
                        .enabled_files
                        .entry(path.clone())
                        .or_insert((false, None));
                    entry.0 = !entry.0;
                    entry.1 = Some(position);
                    let enabled = entry.0;
                    drop(state);

                    if enabled {
                        log::debug!("Publishing ownership diagnostics for {}", path.display());
                        self.publish_ownership_diagnostics(&path, position).await;
                    } else {
                        log::debug!("Clearing ownership diagnostics for {}", path.display());
                        self.clear_ownership_diagnostics(&path).await;
                    }
                    Ok(Some(serde_json::json!({ "enabled": enabled })))
                } else {
                    log::error!("Failed to parse position args from {:?}", params.arguments);
                    Err(jsonrpc::Error::invalid_params(
                        "Expected arguments: [document_uri, line, character]",
                    ))
                }
            }
            CMD_ENABLE_OWNERSHIP => {
                if let Some((path, position)) = Self::parse_position_args(&params.arguments) {
                    let mut state = self.ownership_state.write().await;
                    state
                        .enabled_files
                        .insert(path.clone(), (true, Some(position)));
                    drop(state);
                    self.publish_ownership_diagnostics(&path, position).await;
                    Ok(Some(serde_json::json!({ "enabled": true })))
                } else {
                    Err(jsonrpc::Error::invalid_params(
                        "Expected arguments: [document_uri, line, character]",
                    ))
                }
            }
            CMD_DISABLE_OWNERSHIP => {
                if let Some((path, _)) = Self::parse_position_args(&params.arguments) {
                    let mut state = self.ownership_state.write().await;
                    state.enabled_files.insert(path.clone(), (false, None));
                    drop(state);
                    self.clear_ownership_diagnostics(&path).await;
                    Ok(Some(serde_json::json!({ "enabled": false })))
                } else {
                    Err(jsonrpc::Error::invalid_params(
                        "Expected arguments: [document_uri]",
                    ))
                }
            }
            CMD_ANALYZE => {
                self.do_analyze().await;
                Ok(Some(serde_json::json!({ "status": "analyzing" })))
            }
            _ => Err(jsonrpc::Error::method_not_found()),
        }
    }

    /// Parse position arguments from command: [`uri_string`, line, character]
    fn parse_position_args(args: &[serde_json::Value]) -> Option<(PathBuf, lsp_types::Position)> {
        if args.is_empty() {
            return None;
        }
        let uri_str = args.first()?.as_str()?;
        let uri = lsp_types::Url::parse(uri_str).ok()?;
        let path = uri.to_file_path().ok()?;

        let line = u32::try_from(args.get(1).and_then(serde_json::Value::as_u64).unwrap_or(0))
            .unwrap_or(0);

        let character = u32::try_from(args.get(2).and_then(serde_json::Value::as_u64).unwrap_or(0))
            .unwrap_or(0);

        Some((path, lsp_types::Position { line, character }))
    }

    pub async fn check_with_options(
        path: impl AsRef<Path>,
        all_targets: bool,
        all_features: bool,
    ) -> bool {
        let path = path.as_ref();
        let (service, _) = LspService::build(Self::new).finish();
        let backend = service.inner();

        if backend.add_analyze_target(path).await {
            backend
                .analyze_with_options(all_targets, all_features)
                .await;
            while backend.processes.write().await.join_next().await.is_some() {}
            backend
                .analyzed
                .read()
                .await
                .as_ref()
                .is_some_and(|v| !v.0.is_empty())
        } else {
            false
        }
    }

    pub async fn shutdown_subprocesses(&self) {
        {
            let mut tokens = self.process_tokens.write().await;
            while let Some((_, token)) = tokens.pop_last() {
                token.cancel();
            }
        }
        self.processes.write().await.shutdown().await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: lsp_types::InitializeParams,
    ) -> jsonrpc::Result<lsp_types::InitializeResult> {
        let mut workspaces = Vec::new();
        if let Some(root) = params.root_uri
            && let Ok(path) = root.to_file_path()
        {
            workspaces.push(path);
        }
        if let Some(wss) = params.workspace_folders {
            workspaces.extend(wss.iter().filter_map(|v| v.uri.to_file_path().ok()));
        }
        for path in workspaces {
            self.add_analyze_target(&path).await;
        }
        self.do_analyze().await;

        let sync_options = lsp_types::TextDocumentSyncOptions {
            open_close: Some(true),
            save: Some(lsp_types::TextDocumentSyncSaveOptions::Supported(true)),
            change: Some(lsp_types::TextDocumentSyncKind::INCREMENTAL),
            ..Default::default()
        };
        let workspace_cap = lsp_types::WorkspaceServerCapabilities {
            workspace_folders: Some(lsp_types::WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(lsp_types::OneOf::Left(true)),
            }),
            ..Default::default()
        };
        // Advertise executeCommand capability with supported commands
        let execute_command_provider = lsp_types::ExecuteCommandOptions {
            commands: vec![
                CMD_TOGGLE_OWNERSHIP.to_string(),
                CMD_ENABLE_OWNERSHIP.to_string(),
                CMD_DISABLE_OWNERSHIP.to_string(),
                CMD_ANALYZE.to_string(),
            ],
            work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
        };
        // Advertise code action support
        let code_action_provider = lsp_types::CodeActionProviderCapability::Simple(true);
        let server_cap = lsp_types::ServerCapabilities {
            text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Options(sync_options)),
            workspace: Some(workspace_cap),
            execute_command_provider: Some(execute_command_provider),
            code_action_provider: Some(code_action_provider),
            ..Default::default()
        };
        let init_res = lsp_types::InitializeResult {
            capabilities: server_cap,
            ..Default::default()
        };
        let health_checker = async move {
            if let Some(process_id) = params.process_id {
                loop {
                    time::sleep(time::Duration::from_secs(30)).await;
                    assert!(
                        process_alive::state(process_alive::Pid::from(process_id)).is_alive(),
                        "The client process is dead"
                    );
                }
            }
        };
        if params
            .capabilities
            .window
            .and_then(|v| v.work_done_progress)
            .unwrap_or(false)
        {
            *self.work_done_progress.write().await = true;
        }
        tokio::spawn(health_checker);
        Ok(init_res)
    }

    async fn did_change_workspace_folders(
        &self,
        params: lsp_types::DidChangeWorkspaceFoldersParams,
    ) -> () {
        for added in params.event.added {
            if let Ok(path) = added.uri.to_file_path()
                && self.add_analyze_target(&path).await
            {
                self.do_analyze().await;
            }
        }
    }

    async fn did_open(&self, params: lsp_types::DidOpenTextDocumentParams) {
        if let Ok(path) = params.text_document.uri.to_file_path()
            && path.is_file()
            && params.text_document.language_id == "rust"
            && self.add_analyze_target(&path).await
        {
            self.do_analyze().await;
        }
    }

    async fn did_change(&self, _params: lsp_types::DidChangeTextDocumentParams) {
        *self.analyzed.write().await = None;
        self.shutdown_subprocesses().await;
    }

    async fn code_action(
        &self,
        params: lsp_types::CodeActionParams,
    ) -> jsonrpc::Result<Option<lsp_types::CodeActionResponse>> {
        let uri = params.text_document.uri.clone();
        let position = params.range.start;

        // Check analysis status
        let status = *self.status.read().await;
        let is_analyzed = self.analyzed.read().await.is_some();

        // Check if ownership diagnostics are currently enabled for this file
        let is_enabled = if let Ok(path) = uri.to_file_path() {
            self.ownership_state
                .read()
                .await
                .enabled_files
                .get(&path)
                .is_some_and(|(enabled, _)| *enabled)
        } else {
            false
        };

        let mut actions = Vec::new();

        // Show/Hide ownership action - always show, even if not ready
        let title = match (is_analyzed, is_enabled) {
            _ if status == progress::AnalysisStatus::Analyzing => {
                "FerrousOwl: Show ownership (analyzing...)"
            }
            (false, _) => "FerrousOwl: Show ownership (waiting for analysis)",
            (true, true) => "FerrousOwl: Hide ownership",
            (true, false) => "FerrousOwl: Show ownership",
        };

        let action = lsp_types::CodeAction {
            title: title.to_string(),
            kind: Some(lsp_types::CodeActionKind::SOURCE),
            diagnostics: None,
            edit: None,
            command: Some(lsp_types::Command {
                title: title.to_string(),
                command: CMD_TOGGLE_OWNERSHIP.to_string(),
                arguments: Some(vec![
                    serde_json::json!(uri.to_string()),
                    serde_json::json!(position.line),
                    serde_json::json!(position.character),
                ]),
            }),
            is_preferred: None,
            disabled: None,
            data: None,
        };
        actions.push(lsp_types::CodeActionOrCommand::CodeAction(action));

        Ok(Some(actions))
    }

    async fn execute_command(
        &self,
        params: lsp_types::ExecuteCommandParams,
    ) -> jsonrpc::Result<Option<serde_json::Value>> {
        self.handle_execute_command(params).await
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.shutdown_subprocesses().await;
        Ok(())
    }
}
