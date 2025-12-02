use std::{
    fs,
    io::{Error, ErrorKind, Result},
    path::Path,
    time::Duration,
};

use crate::lsp_client::{file_uri, LspClient};
use crate::types::{ExpectedDeco, ReceivedDiagnostic, TestCase};

/// Result of running a test case.
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

/// Run a single test case against the LSP server.
pub fn run_test(
    client: &mut LspClient,
    test: &TestCase,
    workspace_dir: &str,
) -> Result<TestResult> {
    // Write test source to a temporary file
    let test_file = format!("{workspace_dir}/test_source.rs");
    fs::write(&test_file, &test.source)?;

    let file_uri = file_uri(&test_file);

    // Open the document
    client.open_document(&file_uri, "rust", &test.source)?;

    // Wait for diagnostics
    let diagnostics = client.wait_for_decorations(&test.expected, Duration::from_secs(30))?;

    // Verify results
    let result = verify_decorations(&test.expected, &diagnostics);

    // Clean up
    let _ = fs::remove_file(&test_file);

    Ok(TestResult {
        name: test.name.clone(),
        passed: result.0,
        message: result.1,
    })
}

fn verify_decorations(expected: &[ExpectedDeco], received: &[ReceivedDiagnostic]) -> (bool, String) {
    let mut missing = Vec::new();
    let mut matched = vec![false; received.len()];

    for exp in expected {
        let found = received.iter().enumerate().any(|(i, r)| {
            if r.matches(exp) && !matched[i] {
                matched[i] = true;
                true
            } else {
                false
            }
        });

        if !found {
            missing.push(format!(
                "  {:?} at line {} ({}-{})",
                exp.kind, exp.line, exp.start_char, exp.end_char
            ));
        }
    }

    let unexpected: Vec<_> = received
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched[*i])
        .map(|(_, r)| {
            format!(
                "  {} at line {} ({}-{})",
                r.kind, r.line, r.start_char, r.end_char
            )
        })
        .collect();

    if missing.is_empty() && unexpected.is_empty() {
        (true, "All decorations match".to_string())
    } else {
        let mut msg = String::new();
        if !missing.is_empty() {
            msg.push_str("Missing:\n");
            msg.push_str(&missing.join("\n"));
        }
        if !unexpected.is_empty() {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str("Unexpected:\n");
            msg.push_str(&unexpected.join("\n"));
        }
        (false, msg)
    }
}

/// Set up a workspace directory for testing.
pub fn setup_workspace(base_dir: &str, name: &str) -> Result<String> {
    let workspace_dir = format!("{base_dir}/{name}");
    fs::create_dir_all(&workspace_dir)?;

    // Create a minimal Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
"#
    );
    fs::write(format!("{workspace_dir}/Cargo.toml"), cargo_toml)?;

    // Create src directory
    fs::create_dir_all(format!("{workspace_dir}/src"))?;

    Ok(workspace_dir)
}

/// Clean up a workspace directory.
pub fn cleanup_workspace(workspace_dir: &str) -> Result<()> {
    if Path::new(workspace_dir).exists() {
        fs::remove_dir_all(workspace_dir)?;
    }
    Ok(())
}

/// Load test cases from a JSON file.
#[allow(dead_code, reason = "Utility function for future use")]
pub fn load_tests_from_file(path: &str) -> Result<Vec<TestCase>> {
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|e| Error::new(ErrorKind::InvalidData, e))
}
