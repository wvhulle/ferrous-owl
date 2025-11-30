//! Analysis module for RustOwl LSP
//!
//! This module handles triggering ownership/lifetime analysis using the integrated compiler.

use crate::{cache::*, compiler, models::*, toolchain};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process,
    sync::{Notify, mpsc},
};

#[derive(serde::Deserialize, Clone, Debug)]
pub struct CargoCheckMessageTarget {
    name: String,
}
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum CargoCheckMessage {
    #[allow(unused)]
    CompilerArtifact { target: CargoCheckMessageTarget },
    #[allow(unused)]
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
        } else if path.is_file() && path.extension().map(|v| v == "rs").unwrap_or(false) {
            Ok(Self {
                path,
                metadata: None,
            })
        } else {
            log::warn!("Invalid analysis target: {}", path.display());
            Err(())
        }
    }
    pub fn target_path(&self) -> &Path {
        &self.path
    }
    pub fn workspace_path(&self) -> Option<&Path> {
        if self.metadata.is_some() {
            Some(&self.path)
        } else {
            None
        }
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
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
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
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true);

        if is_cache() {
            set_cache_path(&mut command, target_dir);
        }

        if log::max_level()
            .to_level()
            .map(|v| v < log::Level::Info)
            .unwrap_or(true)
        {
            command.stderr(std::process::Stdio::null());
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
                    log::info!("crate {checked} checked");

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
            log::info!("stdout closed");
            notify_c.notify_one();
        });

        AnalyzeEventIter {
            receiver,
            notify,
            child: Some(child),
        }
    }

    async fn analyze_single_file(&self, path: &Path) -> AnalyzeEventIter {
        let sysroot = toolchain::get_sysroot();
        let path = path.to_path_buf();

        let (sender, receiver) = mpsc::channel(1024);
        let notify = Arc::new(Notify::new());
        let notify_c = notify.clone();

        log::info!("start analyzing {}", path.display());

        // Build args for in-process compiler
        let mut args = vec![
            "rustowl".to_string(), // program name
            "rustowl".to_string(), // triggers workspace wrapper mode
            format!("--sysroot={}", sysroot.display()),
            "--crate-type=lib".to_string(),
        ];
        #[cfg(unix)]
        args.push("-o/dev/null".to_string());
        #[cfg(windows)]
        args.push("-oNUL".to_string());
        args.push(path.to_string_lossy().to_string());

        // Run compiler in a separate thread with panic handling
        let _handle = tokio::task::spawn_blocking(move || {
            let (ws_receiver, compiler_handle) = compiler::run_compiler_in_thread(args);

            // Create a runtime to receive results
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let mut ws_receiver = ws_receiver;
                while let Some(ws) = ws_receiver.recv().await {
                    let event = AnalyzerEvent::Analyzed(ws);
                    if sender.send(event).await.is_err() {
                        break;
                    }
                }
            });

            // Wait for compiler thread to finish
            match compiler_handle.join() {
                Ok(Ok(_)) => log::info!("Compiler finished successfully"),
                Ok(Err(e)) => log::warn!("Compiler error: {}", e),
                Err(_) => log::error!("Compiler thread panicked"),
            }

            notify_c.notify_one();
        });

        AnalyzeEventIter {
            receiver,
            notify,
            child: None,
        }
    }
}

pub struct AnalyzeEventIter {
    receiver: mpsc::Receiver<AnalyzerEvent>,
    notify: Arc<Notify>,
    #[allow(unused)]
    child: Option<process::Child>,
}
impl AnalyzeEventIter {
    pub async fn next_event(&mut self) -> Option<AnalyzerEvent> {
        tokio::select! {
            v = self.receiver.recv() => v,
            _ = self.notify.notified() => None,
        }
    }
}
