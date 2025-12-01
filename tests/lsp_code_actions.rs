//! Integration tests for LSP code actions.
//!
//! Tests the "Show ownership" code action functionality by spawning the ferrous-owl
//! binary and communicating via the LSP protocol over stdio.

use std::{
    env::current_exe,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio, id as process_id},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

const TOGGLE_OWNERSHIP_CMD: &str = "ferrous-owl.toggleOwnership";
const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

struct LspClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    request_id: i64,
}

impl LspClient {
    fn spawn() -> Self {
        let binary = current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(env!("CARGO_PKG_NAME"));

        assert!(binary.exists(), "Binary not found at {}", binary.display());

        let mut child = Command::new(&binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to spawn {}: {e}", binary.display()));

        let stdin = child.stdin.take().expect("Failed to get stdin");
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let reader = BufReader::new(stdout);

        Self {
            child,
            stdin,
            reader,
            request_id: 0,
        }
    }

    fn send_request(&mut self, method: &str, params: &Value) -> Value {
        self.request_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params
        });

        self.send_message(&request);
        self.read_response(self.request_id)
    }

    fn send_notification(&mut self, method: &str, params: &Value) {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.send_message(&notification);
    }

    fn send_message(&mut self, message: &Value) {
        let content = serde_json::to_string(message).unwrap();
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        self.stdin.write_all(header.as_bytes()).unwrap();
        self.stdin.write_all(content.as_bytes()).unwrap();
        self.stdin.flush().unwrap();
    }

    fn read_message(&mut self) -> Value {
        loop {
            let mut header = String::new();
            assert!(
                self.reader.read_line(&mut header).unwrap() != 0,
                "LSP server closed connection unexpectedly"
            );

            if header.trim().is_empty() {
                continue;
            }

            if !header.starts_with("Content-Length:") {
                continue;
            }

            let content_length: usize = header
                .trim_start_matches("Content-Length:")
                .trim()
                .parse()
                .expect("Invalid Content-Length header");

            let mut empty_line = String::new();
            self.reader.read_line(&mut empty_line).unwrap();

            let mut content = vec![0u8; content_length];
            self.reader.read_exact(&mut content).unwrap();

            return serde_json::from_slice(&content).expect("Invalid JSON from server");
        }
    }

    fn read_response(&mut self, expected_id: i64) -> Value {
        loop {
            let response = self.read_message();

            if response.get("id").and_then(Value::as_i64) == Some(expected_id) {
                return response;
            }
        }
    }

    fn initialize(&mut self, root_uri: &str) -> Value {
        let params = json!({
            "processId": process_id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "codeAction": {
                        "dynamicRegistration": false
                    }
                }
            }
        });

        let response = self.send_request("initialize", &params);
        self.send_notification("initialized", &json!({}));
        response
    }

    fn shutdown(&mut self) {
        self.send_request("shutdown", &json!(null));
        self.send_notification("exit", &json!(null));
        thread::sleep(Duration::from_millis(100));
    }

    fn code_action(&mut self, uri: &str, line: u32, character: u32) -> Value {
        let params = json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": line, "character": character },
                "end": { "line": line, "character": character }
            },
            "context": {
                "diagnostics": []
            }
        });

        self.send_request("textDocument/codeAction", &params)
    }

    /// Execute a command and collect any diagnostics notifications that arrive
    /// before or shortly after the response. Returns `(response,
    /// diagnostics_for_uri)`.
    fn execute_command_with_diagnostics(
        &mut self,
        command: &str,
        arguments: &[Value],
        diagnostics_uri: &str,
    ) -> (Value, Vec<Value>) {
        self.request_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "workspace/executeCommand",
            "params": {
                "command": command,
                "arguments": arguments
            }
        });

        self.send_message(&request);

        let expected_id = self.request_id;
        let mut collected_diagnostics = Vec::new();
        let mut response = None;

        let timeout = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                Instant::now() <= timeout,
                "Timeout waiting for command response or diagnostics"
            );

            let message = self.read_message();

            // Check for diagnostics notification
            if message.get("method").and_then(Value::as_str)
                == Some("textDocument/publishDiagnostics")
                && let Some(params) = message.get("params")
                && params.get("uri").and_then(Value::as_str) == Some(diagnostics_uri)
                && let Some(diags) = params.get("diagnostics").and_then(Value::as_array)
            {
                collected_diagnostics.clone_from(diags);
            }

            // Check for response
            if message.get("id").and_then(Value::as_i64) == Some(expected_id) {
                response = Some(message);
            }

            // Return once we have both response and non-empty diagnostics
            if let Some(resp) = response.take() {
                if !collected_diagnostics.is_empty() {
                    return (resp, collected_diagnostics);
                }
                // Put it back if we still need to wait for diagnostics
                response = Some(resp);
                // Give a short grace period for diagnostics to arrive
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    fn get_ferrous_owl_action_title(&mut self, uri: &str, line: u32, character: u32) -> Option<String> {
        let response = self.code_action(uri, line, character);
        response
            .get("result")
            .and_then(Value::as_array)
            .and_then(|a| {
                a.iter().find(|x| {
                    x.get("title")
                        .and_then(Value::as_str)
                        .is_some_and(|t| t.contains("Ferrous"))
                })
            })
            .and_then(|a| a.get("title"))
            .and_then(Value::as_str)
            .map(String::from)
    }

    fn wait_for_analysis(&mut self, uri: &str, line: u32, character: u32) {
        let start = Instant::now();
        loop {
            assert!(
                start.elapsed() <= ANALYSIS_TIMEOUT,
                "Timeout waiting for analysis to complete"
            );

            if let Some(title) = self.get_ferrous_owl_action_title(uri, line, character)
                && !title.contains("analyzing")
                && !title.contains("waiting")
            {
                return;
            }

            thread::sleep(POLL_INTERVAL);
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn test_file_uri() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/dummy/src/lib.rs");
    format!("file://{}", path.display())
}

fn perf_tests_workspace_uri() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/dummy");
    format!("file://{}", path.display())
}

#[test]
fn ownership_diagnostics_show_function_call() {
    let mut client = LspClient::spawn();
    // Use the dummy directory as workspace root so the lib.rs file is analyzed
    client.initialize(&perf_tests_workspace_uri());

    let uri = test_file_uri();

    // Line 173 (0-indexed: 172) contains: `let state = SharedState::new();`
    // Position on `state` variable (character 12)
    let line = 172;
    let character = 12;

    // Wait for analysis to complete
    client.wait_for_analysis(&uri, line, character);

    // Enable ownership display - this triggers publishing diagnostics
    // Use the method that captures diagnostics notifications during the request
    let toggle_args = [json!(uri.clone()), json!(line), json!(character)];
    let (_response, diagnostics) =
        client.execute_command_with_diagnostics(TOGGLE_OWNERSHIP_CMD, &toggle_args, &uri);

    // Verify we got diagnostics
    assert!(
        !diagnostics.is_empty(),
        "Should receive ownership diagnostics after enabling"
    );

    // Look for a `ferrous-owl:call` diagnostic for the SharedState::new() function
    // call
    let call_diagnostic = diagnostics.iter().find(|d| {
        d.get("code")
            .and_then(Value::as_str)
            .is_some_and(|c| c == "ferrous-owl:call")
    });

    assert!(
        call_diagnostic.is_some(),
        "Should have a 'ferrous-owl:call' diagnostic for SharedState::new(). Got diagnostics: \
         {diagnostics:?}"
    );

    let call_diag = call_diagnostic.unwrap();

    // Verify the diagnostic message indicates it's a function call
    let message = call_diag
        .get("message")
        .and_then(Value::as_str)
        .expect("Diagnostic should have message");
    assert!(
        message.contains("function call"),
        "Call diagnostic message should contain 'function call', got: {message}"
    );

    // Verify severity is INFORMATION (3)
    let severity = call_diag
        .get("severity")
        .and_then(Value::as_u64)
        .expect("Diagnostic should have severity");
    assert_eq!(
        severity, 3,
        "Call diagnostic severity should be INFORMATION (3), got: {severity}"
    );

    // Verify the diagnostic is on the correct line (172, 0-indexed)
    let range = call_diag
        .get("range")
        .expect("Diagnostic should have range");
    let start_line = range
        .get("start")
        .and_then(|s| s.get("line"))
        .and_then(Value::as_u64)
        .expect("Range should have start line");
    assert_eq!(
        start_line, 172,
        "Call diagnostic should be on line 172, got: {start_line}"
    );

    client.shutdown();
}
