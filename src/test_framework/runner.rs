//! Test runner utilities for ferrous-owl LSP decoration tests.

use std::{fs, io::Result, path::Path, time::Duration};

use super::{
    TestCase,
    lsp_client::{LspClient, ReceivedDiagnostic, file_uri},
};

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
    let test_file = format!("{workspace_dir}/test_source.rs");
    fs::write(&test_file, &test.code)?;

    let file_uri = file_uri(&test_file);

    client.open_document(&file_uri, "rust", &test.code)?;

    let (line, character) = resolve_cursor_position(test);
    log::info!("Using cursor position: line={line}, char={character}");

    client.wait_for_analysis(&file_uri, line, character, Duration::from_secs(30))?;

    let diagnostics =
        client.toggle_ownership_and_wait(&file_uri, line, character, Duration::from_secs(10))?;
    log::info!("Got {} diagnostics, verifying...", diagnostics.len());

    let result = verify_decorations(test, &diagnostics);
    log::info!("Verification complete: passed={}", result.0);

    let _ = fs::remove_file(&test_file);
    log::info!("Test file cleaned up");

    Ok(TestResult {
        name: test.name.clone(),
        passed: result.0,
        message: result.1,
    })
}

/// Resolve the cursor position from the test case.
/// Returns (line, character) as 0-indexed values.
fn resolve_cursor_position(test: &TestCase) -> (u32, u32) {
    if let (Some(line), Some(char)) = (test.cursor_line, test.cursor_char) {
        return (line, char);
    }

    if let Some(ref text) = test.cursor_text {
        for (line_idx, line_content) in test.code.lines().enumerate() {
            if let Some(col) = line_content.find(text) {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "line/column indices fit in u32"
                )]
                return (line_idx as u32, col as u32);
            }
        }
        log::warn!("cursor_text '{text}' not found in code, defaulting to (0, 0)");
    }

    (0, 0)
}

fn verify_decorations(test: &TestCase, received: &[ReceivedDiagnostic]) -> (bool, String) {
    let expected = &test.expected_decos;
    let forbidden = &test.forbidden_decos;

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
            missing.push(format!("Expected {exp:?} not found."));
        }
    }

    let mut forbidden_found = Vec::new();
    for kind in forbidden {
        for r in received {
            if r.code.ends_with(&format!(":{kind}")) {
                forbidden_found.push(format!(
                    "Forbidden {kind} found at line {} '{}'",
                    r.line, r.message
                ));
            }
        }
    }

    let unexpected: Vec<_> = received
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched[*i])
        .map(|(_, r)| format!("  {} at line {} '{}'", r.code, r.line, r.message))
        .collect();

    if missing.is_empty() && forbidden_found.is_empty() {
        (true, "All decorations match".to_string())
    } else {
        let mut msg = String::new();
        if !missing.is_empty() {
            msg.push_str("Missing:\n");
            msg.push_str(&missing.join("\n"));
        }
        if !forbidden_found.is_empty() {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str("Forbidden:\n");
            msg.push_str(&forbidden_found.join("\n"));
        }
        if !unexpected.is_empty() {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str("Received:\n");
            msg.push_str(&unexpected.join("\n"));
        }
        (false, msg)
    }
}

/// Set up a workspace directory for testing.
pub fn setup_workspace(base_dir: &str, name: &str) -> Result<String> {
    let workspace_dir = format!("{base_dir}/{name}");
    fs::create_dir_all(&workspace_dir)?;

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
"#
    );
    fs::write(format!("{workspace_dir}/Cargo.toml"), cargo_toml)?;

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
