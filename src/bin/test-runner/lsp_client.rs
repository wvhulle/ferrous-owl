use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Read, Result, Write},
    process::{id as process_id, Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde_json::{json, Value};

use crate::types::{ExpectedDeco, ReceivedDiagnostic};

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
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

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
        let id = self.send_request("shutdown", &json!(null))?;
        let _ = self.wait_for_response(id, Duration::from_secs(5));
        self.send_notification("exit", &json!(null))?;
        let _ = self.child.wait();
        Ok(())
    }

    /// Wait for diagnostics and collect decoration-related ones.
    pub fn wait_for_decorations(
        &mut self,
        expected: &[ExpectedDeco],
        timeout: Duration,
    ) -> Result<Vec<ReceivedDiagnostic>> {
        let start = Instant::now();
        let mut diagnostics = Vec::new();

        while start.elapsed() < timeout && diagnostics.len() < expected.len() {
            if let Some(msg) = self.receive_message(Duration::from_millis(100))?
                && msg.get("method").and_then(Value::as_str)
                    == Some("textDocument/publishDiagnostics")
                && let Some(params) = msg.get("params")
                && let Some(diag_array) = params.get("diagnostics").and_then(Value::as_array)
            {
                for diag in diag_array {
                    if let Some(received) = ReceivedDiagnostic::from_lsp(diag) {
                        diagnostics.push(received);
                    }
                }
            }
        }

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

        // Skip the empty line after Content-Length
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
pub fn file_uri(path: &str) -> String {
    format!("file://{path}")
}

/// Read file contents.
#[allow(dead_code, reason = "Utility function for future use")]
pub fn read_file(path: &str) -> Result<String> {
    fs::read_to_string(path)
}
