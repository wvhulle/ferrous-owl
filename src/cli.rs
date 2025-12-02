use std::{env, path::PathBuf, process::exit};

use clap::{ArgAction, Args, Parser, Subcommand, ValueHint};
use tokio::{fs::remove_dir_all, io};
use tower_lsp::{LspService, Server};

use crate::lsp_server::Backend;

#[derive(Debug, Parser)]
#[command(author)]
pub struct Cli {
    /// Print version.
    #[arg(short('V'), long)]
    pub version: bool,

    /// Suppress output.
    #[arg(short, long, action(ArgAction::Count))]
    pub quiet: u8,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check availability.
    Check(Check),

    /// Remove artifacts from the target directory.
    Clean,
}

#[derive(Args, Debug)]
pub struct Check {
    /// The path of a file or directory to check availability.
    #[arg(value_name("path"), value_hint(ValueHint::AnyPath))]
    pub path: Option<PathBuf>,

    /// Whether to check for all targets
    /// (default: false).
    #[arg(
        long,
        default_value_t = false,
        value_name("all-targets"),
        help = "Run the check for all targets instead of current only"
    )]
    pub all_targets: bool,

    /// Whether to check for all features
    /// (default: false).
    #[arg(
        long,
        default_value_t = false,
        value_name("all-features"),
        help = "Run the check for all features instead of the current active ones only"
    )]
    pub all_features: bool,
}

impl Commands {
    /// Execute the command.
    pub async fn execute(self) {
        match self {
            Self::Check(options) => {
                let path = options.path.unwrap_or_else(|| env::current_dir().unwrap());

                if Backend::check_with_options(&path, options.all_targets, options.all_features)
                    .await
                {
                    log::info!("Successfully analyzed");
                    exit(0);
                }
                log::error!("Analyze failed");
                exit(1);
            }
            Self::Clean => {
                if let Ok(meta) = cargo_metadata::MetadataCommand::new().exec() {
                    let target = meta.target_directory.join("owl");
                    remove_dir_all(&target).await.ok();
                }
            }
        }
    }
}

impl Cli {
    /// Run the CLI application.
    pub async fn run(self) {
        if let Some(command) = self.command {
            command.execute().await;
        } else if self.version {
            if self.quiet == 0 {
                print!("{} ", env!("CARGO_PKG_NAME"));
            }
            println!("v{}", clap::crate_version!());
        } else {
            start_lsp_server().await;
        }
    }
}

async fn start_lsp_server() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("ferrous-owl/cursor", Backend::cursor)
        .custom_method("ferrous-owl/analyze", Backend::analyze)
        .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
