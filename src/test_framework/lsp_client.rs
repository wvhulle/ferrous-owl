//! LSP JSON-RPC client for testing the ferrous-owl language server.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Read, Result, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio, id as process_id},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::ExpectedDeco;
use crate::models::Loc;

/// Received diagnostic from LSP.
#[derive(Debug, Clone)]
pub struct ReceivedDiagnostic {
    pub code: String,
    pub line: Loc,
    pub message: String,
}

impl ReceivedDiagnostic {
    /// Parse from LSP diagnostic JSON.
    pub fn from_lsp(value: &Value) -> Option<Self> {
        let code = value.get("code")?.as_str()?;
        let range = value.get("range")?;
        let start = range.get("start")?;
        let message = value.get("message").and_then(Value::as_str).unwrap_or("");

        let line = Loc::from(start.get("line")?.as_u64()?);

        Some(Self {
            code: code.to_string(),
            line,
            message: message.to_string(),
        })
    }

    /// Check if this diagnostic matches an expected decoration.
    #[must_use]
    pub fn matches(&self, expected: &ExpectedDeco) -> bool {
        // Check kind matches (code contains the kind, e.g., "ferrous-owl:mut-borrow")
        let kind_matches = self.code.ends_with(&format!(":{}", expected.kind));

        // Check line if specified
        let line_matches = expected.line.is_none_or(|l| self.line == Loc::from(l));

        // Check text_match if specified (look in message)
        let text_matches = expected
            .text_match
            .as_ref()
            .is_none_or(|t| self.message.contains(t));

        // Check message_contains if specified
        let message_matches = expected
            .message_contains
            .as_ref()
            .is_none_or(|m| self.message.contains(m));

        kind_matches && line_matches && text_matches && message_matches
    }
}

/// LSP JSON-RPC client for testing the ferrous-owl language server.
pub struct LspClient {
    child: Child,
    writer: BufWriter<ChildStdin>,
    receiver: Receiver<Value>,
    _reader_thread: JoinHandle<()>,
    request_id: i64,
    pending_requests: HashMap<i64, String>,
}

impl LspClient {
    /// Start a new LSP server process.
    pub fn start(command: &str, args: &[&str]) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::other("Failed to get stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::other("Failed to get stdout"))?;

        let writer = BufWriter::new(stdin);
        let (sender, receiver) = mpsc::channel();

        let reader_thread = thread::spawn(move || {
            read_messages(stdout, &sender);
        });

        Ok(Self {
            child,
            writer,
            receiver,
            _reader_thread: reader_thread,
            request_id: 0,
            pending_requests: HashMap::new(),
        })
    }

    /// Send an LSP request and return the request ID.
    pub fn send_request(&mut self, method: &str, params: &Value) -> Result<i64> {
        self.request_id += 1;
        let id = self.request_id;

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        self.pending_requests.insert(id, method.to_string());
        self.send_message(&request)?;
        Ok(id)
    }

    /// Send an LSP notification (no response expected).
    pub fn send_notification(&mut self, method: &str, params: &Value) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.send_message(&notification)
    }

    fn send_message(&mut self, message: &Value) -> Result<()> {
        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        self.writer.write_all(header.as_bytes())?;
        self.writer.write_all(content.as_bytes())?;
        self.writer.flush()?;

        Ok(())
    }

    /// Receive the next message with a timeout.
    pub fn receive_message(&mut self, timeout: Duration) -> Result<Option<Value>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::new(
                ErrorKind::BrokenPipe,
                "Reader thread disconnected",
            )),
        }
    }

    /// Wait for a response to a specific request ID.
    pub fn wait_for_response(&mut self, id: i64, timeout: Duration) -> Result<Value> {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if let Some(msg) = self.receive_message(Duration::from_millis(100))?
                && let Some(response_id) = msg.get("id").and_then(Value::as_i64)
                && response_id == id
            {
                self.pending_requests.remove(&id);
                return Ok(msg);
            }
        }

        Err(Error::new(
            ErrorKind::TimedOut,
            format!("Timeout waiting for response to request {id}"),
        ))
    }

    /// Initialize the LSP connection with standard capabilities.
    pub fn initialize(&mut self, root_uri: &str) -> Result<Value> {
        let params = json!({
            "processId": process_id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "publishDiagnostics": {
                        "relatedInformation": true
                    },
                    "codeAction": {
                        "codeActionLiteralSupport": {
                            "codeActionKind": {
                                "valueSet": ["quickfix", "refactor"]
                            }
                        }
                    }
                }
            }
        });

        let id = self.send_request("initialize", &params)?;
        let response = self.wait_for_response(id, Duration::from_secs(30))?;

        self.send_notification("initialized", &json!({}))?;

        Ok(response)
    }

    /// Open a text document in the server.
    pub fn open_document(&mut self, uri: &str, language_id: &str, text: &str) -> Result<()> {
        self.send_notification(
            "textDocument/didOpen",
            &json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        )
    }

    /// Request shutdown and exit.
    pub fn shutdown(&mut self) -> Result<()> {
        log::debug!("Sending shutdown request...");
        let id = self.send_request("shutdown", &json!(null))?;
        let _ = self.wait_for_response(id, Duration::from_secs(2));
        log::debug!("Sending exit notification...");
        self.send_notification("exit", &json!(null))?;
        log::debug!("Killing child process...");
        let _ = self.child.kill();
        let _ = self.child.wait();
        log::debug!("Shutdown complete");
        Ok(())
    }

    /// Request code actions at a position.
    pub fn code_action(&mut self, uri: &str, line: u32, character: u32) -> Result<Value> {
        let id = self.send_request(
            "textDocument/codeAction",
            &json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character }
                },
                "context": { "diagnostics": [] }
            }),
        )?;
        self.wait_for_response(id, Duration::from_secs(30))
    }

    /// Execute a command.
    pub fn execute_command(&mut self, command: &str, args: &[Value]) -> Result<i64> {
        self.send_request(
            "workspace/executeCommand",
            &json!({
                "command": command,
                "arguments": args
            }),
        )
    }

    /// Wait for analysis to complete by polling code actions.
    pub fn wait_for_analysis(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        timeout: Duration,
    ) -> Result<()> {
        log::info!("Waiting for analysis to complete...");
        let start = Instant::now();
        while start.elapsed() < timeout {
            log::debug!("Sending code action request...");
            let response = self.code_action(uri, line, character)?;
            log::debug!("Got code action response: {response:?}");
            if let Some(actions) = response.get("result").and_then(Value::as_array) {
                log::debug!("Got {} actions", actions.len());
                // Check if any action title indicates analysis is complete (not "analyzing" or
                // "waiting")
                let ready = actions.iter().any(|a| {
                    let title = a.get("title").and_then(Value::as_str);
                    log::debug!("Action title: {title:?}");
                    title.is_some_and(|t| {
                        t.contains("Ferrous") && !t.contains("analyzing") && !t.contains("waiting")
                    })
                });
                if ready {
                    log::info!("Analysis complete!");
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
        Err(Error::new(
            ErrorKind::TimedOut,
            "Timeout waiting for analysis",
        ))
    }

    /// Execute toggle ownership command and wait for diagnostics.
    pub fn toggle_ownership_and_wait(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedDiagnostic>> {
        log::info!("Toggling ownership at line={line}, char={character}");
        let cmd_id = self.execute_command(
            "ferrous-owl.toggleOwnership",
            &[json!(uri), json!(line), json!(character)],
        )?;
        log::debug!("Execute command request id: {cmd_id}");

        let start = Instant::now();
        let mut diagnostics = Vec::new();
        let mut got_response = false;

        while start.elapsed() < timeout {
            if let Some(msg) = self.receive_message(Duration::from_millis(100))? {
                log::debug!(
                    "Received message: {:?}",
                    msg.get("method").or_else(|| msg.get("id"))
                );
                if msg.get("id").and_then(Value::as_i64) == Some(cmd_id) {
                    log::info!("Got command response");
                    got_response = true;
                }
                if msg.get("method").and_then(Value::as_str)
                    == Some("textDocument/publishDiagnostics")
                    && let Some(params) = msg.get("params")
                    && let Some(diag_array) = params.get("diagnostics").and_then(Value::as_array)
                {
                    log::info!("Got {} diagnostics", diag_array.len());
                    for diag in diag_array {
                        if let Some(received) = ReceivedDiagnostic::from_lsp(diag) {
                            diagnostics.push(received);
                        }
                    }
                }

                if got_response {
                    thread::sleep(Duration::from_millis(200));
                    while let Some(msg) = self.receive_message(Duration::from_millis(50))? {
                        if msg.get("method").and_then(Value::as_str)
                            == Some("textDocument/publishDiagnostics")
                            && let Some(params) = msg.get("params")
                            && let Some(diag_array) =
                                params.get("diagnostics").and_then(Value::as_array)
                        {
                            for diag in diag_array {
                                if let Some(received) = ReceivedDiagnostic::from_lsp(diag) {
                                    diagnostics.push(received);
                                }
                            }
                        }
                    }
                    log::info!("Returning {} total diagnostics", diagnostics.len());
                    return Ok(diagnostics);
                }
            }
        }

        log::warn!("Timeout waiting for toggle response");
        Ok(diagnostics)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Background reader function that runs in a separate thread.
fn read_messages(stdout: ChildStdout, sender: &Sender<Value>) {
    let mut reader = BufReader::new(stdout);

    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            break;
        }

        let content_length = parse_content_length(&header);
        if content_length == 0 {
            continue;
        }

        let mut empty = String::new();
        if reader.read_line(&mut empty).unwrap_or(0) == 0 {
            break;
        }

        let mut content = vec![0u8; content_length];
        if reader.read_exact(&mut content).is_err() {
            break;
        }

        if let Ok(msg) = serde_json::from_slice::<Value>(&content)
            && sender.send(msg).is_err()
        {
            break;
        }
    }
}

fn parse_content_length(header: &str) -> usize {
    header
        .trim()
        .strip_prefix("Content-Length: ")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Create a file URI from a path.
#[must_use]
pub fn file_uri(path: &str) -> String {
    format!("file://{path}")
}
