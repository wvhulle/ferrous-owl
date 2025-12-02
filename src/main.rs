#![feature(rustc_private)]

use std::{env, process::exit};

use clap::Parser;
use ferrous_owl::{Cli, run_as_rustc_wrapper};

fn initialize_logging() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .target(env_logger::Target::Stderr)
        .init();
}

/// Check if invoked as a compiler by cargo.
///
/// When `setup_cargo_command` spawns cargo with this binary as the compiler,
/// it sets `FERROUS_OWL_AS_RUSTC=1`.
fn is_invoked_as_compiler() -> bool {
    env::var("FERROUS_OWL_AS_RUSTC").is_ok()
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "windows")]
    rayon::ThreadPoolBuilder::new()
        .stack_size(4 * 1024 * 1024)
        .build_global()
        .unwrap();

    if is_invoked_as_compiler() {
        initialize_logging();
        exit(run_as_rustc_wrapper());
    }

    initialize_logging();
    Cli::parse().run().await;
}
