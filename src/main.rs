//! # RustOwl
//!
//! An LSP server and compiler for visualizing ownership and lifetimes in Rust,
//! designed for debugging and optimization.

#![feature(rustc_private)]

use clap::Parser;
use rustowl::cli::Cli;
use std::env;

fn initialize_logging() {
    simple_logger::SimpleLogger::new()
        .with_colors(true)
        .init()
        .unwrap();

    let level = env::var("RUST_LOG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(log::LevelFilter::Info);
    log::set_max_level(level);
}

/// Check if invoked as a compiler by cargo (via RUSTC_WORKSPACE_WRAPPER).
///
/// Cargo passes the executable path as both argv[0] and argv[1] when using workspace wrappers.
fn is_invoked_by_cargo_as_compiler() -> bool {
    let args: Vec<String> = env::args().collect();
    args.first() == args.get(1)
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "windows")]
    rayon::ThreadPoolBuilder::new()
        .stack_size(4 * 1024 * 1024)
        .build_global()
        .unwrap();

    if is_invoked_by_cargo_as_compiler() {
        initialize_logging();
        let args: Vec<String> = env::args().collect();
        std::process::exit(rustowl::compiler::run_compiler_with_args(&args));
    }

    initialize_logging();
    Cli::parse().run().await;
}
