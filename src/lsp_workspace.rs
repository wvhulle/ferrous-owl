use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{self, Command},
    sync::{Notify, mpsc},
    task,
};

use crate::{models::Workspace, rustc_wrapper as compiler, toolchain};

fn set_cache_path(cmd: &mut Command, target_dir: impl AsRef<Path>) {
    cmd.env(toolchain::CACHE_DIR_ENV, target_dir.as_ref().join("cache"));
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct CargoCheckMessageTarget {
    name: String,
}
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum CargoCheckMessage {
    CompilerArtifact { target: CargoCheckMessageTarget },
    BuildFinished {},
}

pub enum AnalyzerEvent {
    CrateChecked {
        package: String,
        package_count: usize,
    },
    Analyzed(Workspace),
}

#[derive(Clone)]
pub struct Analyzer {
    path: PathBuf,
    metadata: Option<cargo_metadata::Metadata>,
}

impl Analyzer {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, ()> {
        let path = path.as_ref().to_path_buf();

        let mut cargo_cmd = toolchain::setup_cargo_command();

        cargo_cmd
            .args([
                "metadata".to_owned(),
                "--filter-platform".to_owned(),
                toolchain::HOST_TUPLE.to_owned(),
            ])
            .current_dir(if path.is_file() {
                path.parent().unwrap()
            } else {
                &path
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let metadata = if let Ok(child) = cargo_cmd.spawn()
            && let Ok(output) = child.wait_with_output().await
        {
            let data = String::from_utf8_lossy(&output.stdout);
            cargo_metadata::MetadataCommand::parse(data).ok()
        } else {
            None
        };

        if let Some(metadata) = metadata {
            Ok(Self {
                path: metadata.workspace_root.as_std_path().to_path_buf(),
                metadata: Some(metadata),
            })
        } else if path.is_file() && path.extension().is_some_and(|v| v == "rs") {
            Ok(Self {
                path,
                metadata: None,
            })
        } else {
            log::warn!("Invalid analysis target: {}", path.display());
            Err(())
        }
    }
    #[must_use]
    pub fn target_path(&self) -> &Path {
        &self.path
    }

    pub async fn analyze(&self, all_targets: bool, all_features: bool) -> AnalyzeEventIter {
        if let Some(metadata) = &self.metadata
            && metadata.root_package().is_some()
        {
            self.analyze_package(metadata, all_targets, all_features)
                .await
        } else {
            self.analyze_single_file(&self.path).await
        }
    }

    async fn analyze_package(
        &self,
        metadata: &cargo_metadata::Metadata,
        all_targets: bool,
        all_features: bool,
    ) -> AnalyzeEventIter {
        let package_name = metadata.root_package().as_ref().unwrap().name.to_string();
        let target_dir = metadata.target_directory.as_std_path().join("owl");
        log::info!("clear cargo cache");
        let mut command = toolchain::setup_cargo_command();
        command
            .args(["clean", "--package", &package_name])
            .env("CARGO_TARGET_DIR", &target_dir)
            .current_dir(&self.path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.spawn().unwrap().wait().await.ok();

        let mut command = toolchain::setup_cargo_command();

        let mut args = vec!["check", "--workspace"];
        if all_targets {
            args.push("--all-targets");
        }
        if all_features {
            args.push("--all-features");
        }
        args.extend_from_slice(&["--keep-going", "--message-format=json"]);

        command
            .args(args)
            .env("CARGO_TARGET_DIR", &target_dir)
            .env_remove("RUSTC_WRAPPER")
            .current_dir(&self.path)
            .stdout(Stdio::piped())
            .kill_on_drop(true);

        set_cache_path(&mut command, target_dir);

        if log::max_level()
            .to_level()
            .is_none_or(|v| v < log::Level::Info)
        {
            command.stderr(Stdio::null());
        }

        let package_count = metadata.packages.len();

        log::info!("start analyzing package {package_name}");
        let mut child = command.spawn().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();

        let (sender, receiver) = mpsc::channel(1024);
        let notify = Arc::new(Notify::new());
        let notify_c = notify.clone();
        let _handle = tokio::spawn(async move {
            // prevent command from dropped
            while let Ok(Some(line)) = stdout.next_line().await {
                if let Ok(CargoCheckMessage::CompilerArtifact { target }) =
                    serde_json::from_str(&line)
                {
                    let checked = target.name;
                    log::debug!("crate {checked} checked");

                    let event = AnalyzerEvent::CrateChecked {
                        package: checked,
                        package_count,
                    };
                    let _ = sender.send(event).await;
                }
                if let Ok(ws) = serde_json::from_str::<Workspace>(&line) {
                    let event = AnalyzerEvent::Analyzed(ws);
                    let _ = sender.send(event).await;
                }
            }
            log::debug!("stdout closed");
            notify_c.notify_one();
        });

        AnalyzeEventIter {
            receiver,
            notify,
            _child: Some(child),
        }
    }

    #[allow(clippy::unused_async, reason = "required by async closure signature")]
    async fn analyze_single_file(&self, path: &Path) -> AnalyzeEventIter {
        let sysroot = toolchain::get_sysroot();
        let path = path.to_path_buf();

        let (sender, receiver) = mpsc::channel(1024);
        let notify = Arc::new(Notify::new());
        let notify_c = notify.clone();

        log::info!("start analyzing {}", path.display());

        let _handle = tokio::spawn(async move {
            let handle = compiler::spawn_analysis(&path, &sysroot);

            let compiler::AnalysisHandle {
                mut results,
                thread,
            } = handle;
            while let Some(ws) = results.recv().await {
                let event = AnalyzerEvent::Analyzed(ws);
                if sender.send(event).await.is_err() {
                    break;
                }
            }

            let join_result = task::spawn_blocking(move || thread.join()).await;
            match join_result {
                Ok(Ok(Ok(_))) => log::info!("Compiler finished successfully"),
                Ok(Ok(Err(e))) => log::warn!("Compiler error: {e}"),
                Ok(Err(_)) => log::error!("Compiler thread panicked"),
                Err(e) => log::error!("Join task failed: {e}"),
            }

            notify_c.notify_one();
        });

        AnalyzeEventIter {
            receiver,
            notify,
            _child: None,
        }
    }
}

pub struct AnalyzeEventIter {
    receiver: mpsc::Receiver<AnalyzerEvent>,
    notify: Arc<Notify>,
    _child: Option<process::Child>,
}
impl AnalyzeEventIter {
    pub async fn next_event(&mut self) -> Option<AnalyzerEvent> {
        tokio::select! {
            v = self.receiver.recv() => v,
            () = self.notify.notified() => None,
        }
    }
}
