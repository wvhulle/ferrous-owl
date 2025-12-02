use std::{
    env,
    fs,
    io::{self, BufRead, Error, ErrorKind, Result},
    path::Path,
    process::exit,
};

mod lsp_client;
mod runner;
mod types;

use lsp_client::LspClient;
use runner::{cleanup_workspace, run_test, setup_workspace};
use types::TestCase;

fn main() -> Result<()> {
    // Read test cases from stdin (JSON format)
    let stdin = io::stdin();
    let mut input = String::new();

    for line in stdin.lock().lines() {
        let line = line?;
        input.push_str(&line);
        input.push('\n');
    }

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

    // Set up workspace
    let base_dir = env::temp_dir()
        .join("owl-tests")
        .to_string_lossy()
        .to_string();
    fs::create_dir_all(&base_dir)?;

    let workspace_dir = setup_workspace(&base_dir, "test_workspace")?;

    // Start LSP server
    let mut client = LspClient::start(&owl_binary, &["lsp"])?;

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
                    println!("✓ {}", result.name);
                } else {
                    failed += 1;
                    println!("✗ {}", result.name);
                    println!("{}", result.message);
                }
                results.push(result);
            }
            Err(e) => {
                failed += 1;
                println!("✗ {} - Error: {e}", test.name);
            }
        }
    }

    // Cleanup
    let _ = client.shutdown();
    let _ = cleanup_workspace(&workspace_dir);

    // Summary
    println!("\n{passed} passed, {failed} failed");

    // Output results as JSON for programmatic consumption
    let output = serde_json::json!({
        "passed": passed,
        "failed": failed,
        "total": tests.len(),
        "results": results.iter().map(|r| {
            serde_json::json!({
                "name": r.name,
                "passed": r.passed,
                "message": r.message
            })
        }).collect::<Vec<_>>()
    });

    println!("\n---JSON---");
    println!("{}", serde_json::to_string_pretty(&output)?);

    if failed > 0 {
        exit(1);
    }

    Ok(())
}

fn find_owl_binary() -> Result<String> {
    // Try to find ferrous-owl binary in common locations
    let candidates = [
        // Same directory as test-runner
        env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("ferrous-owl")))
            .map(|p| p.to_string_lossy().to_string()),
        // Cargo target directory
        Some("target/debug/ferrous-owl".to_string()),
        Some("target/release/ferrous-owl".to_string()),
    ];

    for candidate in candidates.into_iter().flatten() {
        if Path::new(&candidate).exists() {
            return Ok(candidate);
        }
    }

    Err(Error::new(
        ErrorKind::NotFound,
        "Could not find ferrous-owl binary",
    ))
}
