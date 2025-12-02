#![feature(rustc_private)]

use std::{
    env, fs,
    io::{self, BufRead, Error, ErrorKind, Result},
    process::{self, exit},
};

use clap::Parser;
use ferrous_owl::{LspClient, TestCase, cleanup_workspace, run_test, setup_workspace};

/// Test runner for ferrous-owl LSP decoration tests.
#[derive(Parser, Debug)]
#[command(name = "test-runner")]
#[command(about = "Runs LSP decoration tests against ferrous-owl")]
struct Args {
    /// Run a single test case (JSON string)
    #[arg(long)]
    single: Option<String>,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    // Get test input either from --single arg or stdin
    let input = if let Some(json) = args.single {
        format!("[{json}]")
    } else {
        let stdin = io::stdin();
        let mut input = String::new();
        for line in stdin.lock().lines() {
            let line = line?;
            input.push_str(&line);
            input.push('\n');
        }
        input
    };

    // Parse test cases
    let tests: Vec<TestCase> = match serde_json::from_str(&input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to parse test cases: {e}");
            exit(1);
        }
    };

    if tests.is_empty() {
        eprintln!("No test cases provided");
        exit(1);
    }

    // Find the test-runner binary's directory to locate ferrous-owl
    let owl_binary = find_owl_binary()?;

    // Set up workspace with unique name based on test name and process ID to avoid
    // conflicts
    let test_name = tests.first().map_or("test", |t| t.name.as_str());
    let unique_id = process::id();
    let base_dir = env::temp_dir()
        .join("owl-tests")
        .to_string_lossy()
        .to_string();
    fs::create_dir_all(&base_dir)?;

    let workspace_name = format!("{test_name}_{unique_id}");
    let workspace_dir = setup_workspace(&base_dir, &workspace_name)?;

    // Start LSP server (no arguments needed - ferrous-owl starts LSP by default)
    let mut client = LspClient::start(&owl_binary, &[])?;

    // Initialize
    let workspace_uri = format!("file://{workspace_dir}");
    client.initialize(&workspace_uri)?;

    // Run tests
    let mut passed = 0;
    let mut failed = 0;
    let mut results = Vec::new();

    for test in &tests {
        match run_test(&mut client, test, &workspace_dir) {
            Ok(result) => {
                if result.passed {
                    passed += 1;
                    eprintln!("✓ {}", result.name);
                } else {
                    failed += 1;
                    eprintln!("✗ {}", result.name);
                    eprintln!("{}", result.message);
                }
                results.push(result);
            }
            Err(e) => {
                failed += 1;
                eprintln!("✗ {} - Error: {e}", test.name);
            }
        }
    }

    // Cleanup
    log::info!("Shutting down LSP client...");
    let _ = client.shutdown();
    log::info!("Cleaning up workspace...");
    let _ = cleanup_workspace(&workspace_dir);
    log::info!("Cleanup complete");

    // Summary
    eprintln!("\n{passed} passed, {failed} failed");

    if failed > 0 {
        exit(1);
    }

    Ok(())
}

fn find_owl_binary() -> Result<String> {
    // ferrous-owl binary is in the same directory as test-runner
    let owl_path = env::current_exe()?
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "Cannot determine executable directory"))?
        .join("ferrous-owl");

    if owl_path.exists() {
        Ok(owl_path.to_string_lossy().to_string())
    } else {
        Err(Error::new(
            ErrorKind::NotFound,
            format!("ferrous-owl binary not found at {}", owl_path.display()),
        ))
    }
}
