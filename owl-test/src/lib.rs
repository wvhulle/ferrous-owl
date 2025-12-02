//! Test utilities for ferrous-owl LSP decoration tests.
//!
//! This crate provides test case definitions and a test runner that spawns
//! the ferrous-owl test-runner binary. It does NOT depend on rustc_private,
//! so it can be used in normal integration tests.
//!
//! # Example
//!
//! ```rust,ignore
//! use owl_test::{TestCase, DecoKind};
//!
//! #[test]
//! fn test_move_to_drop() {
//!     TestCase::new("move_to_drop", r#"
//!         fn test() {
//!             let s = String::new();
//!             drop(s);
//!         }
//!     "#)
//!     .cursor_on("s = String")
//!     .expect_move()
//!     .run();
//! }
//! ```

use std::{
    env,
    fmt,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecoKind {
    Lifetime,
    ImmBorrow,
    MutBorrow,
    Move,
    Call,
    SharedMut,
    Outlive,
}

impl fmt::Display for DecoKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifetime => write!(f, "lifetime"),
            Self::ImmBorrow => write!(f, "imm-borrow"),
            Self::MutBorrow => write!(f, "mut-borrow"),
            Self::Move => write!(f, "move"),
            Self::Call => write!(f, "call"),
            Self::SharedMut => write!(f, "shared-mut"),
            Self::Outlive => write!(f, "outlive"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedDeco {
    pub kind: DecoKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_match: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_contains: Option<String>,
}

impl ExpectedDeco {
    #[must_use]
    pub const fn new(kind: DecoKind) -> Self {
        Self {
            kind,
            text_match: None,
            line: None,
            message_contains: None,
        }
    }

    #[must_use]
    pub fn at_text(mut self, text: &str) -> Self {
        self.text_match = Some(text.to_string());
        self
    }

    #[must_use]
    pub const fn on_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    #[must_use]
    pub fn with_message(mut self, text: &str) -> Self {
        self.message_contains = Some(text.to_string());
        self
    }

    #[must_use]
    pub const fn move_deco() -> Self {
        Self::new(DecoKind::Move)
    }

    #[must_use]
    pub const fn imm_borrow() -> Self {
        Self::new(DecoKind::ImmBorrow)
    }

    #[must_use]
    pub const fn mut_borrow() -> Self {
        Self::new(DecoKind::MutBorrow)
    }

    #[must_use]
    pub const fn call() -> Self {
        Self::new(DecoKind::Call)
    }

    #[must_use]
    pub const fn lifetime() -> Self {
        Self::new(DecoKind::Lifetime)
    }

    #[must_use]
    pub const fn shared_mut() -> Self {
        Self::new(DecoKind::SharedMut)
    }

    #[must_use]
    pub const fn outlive() -> Self {
        Self::new(DecoKind::Outlive)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_char: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_decos: Vec<ExpectedDeco>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_decos: Vec<DecoKind>,
}

impl TestCase {
    #[must_use]
    pub fn new(name: &str, code: &str) -> Self {
        Self {
            name: name.to_string(),
            code: dedent(code),
            cursor_text: None,
            cursor_line: None,
            cursor_char: None,
            expected_decos: Vec::new(),
            forbidden_decos: Vec::new(),
        }
    }

    #[must_use]
    pub fn cursor_on(mut self, text: &str) -> Self {
        self.cursor_text = Some(text.to_string());
        self
    }

    #[must_use]
    pub const fn cursor_at(mut self, line: u32, character: u32) -> Self {
        self.cursor_line = Some(line);
        self.cursor_char = Some(character);
        self
    }

    #[must_use]
    pub fn expect(mut self, deco: ExpectedDeco) -> Self {
        self.expected_decos.push(deco);
        self
    }

    #[must_use]
    pub fn expect_move(self) -> Self {
        self.expect(ExpectedDeco::move_deco())
    }

    #[must_use]
    pub fn expect_move_at(self, text: &str) -> Self {
        self.expect(ExpectedDeco::move_deco().at_text(text))
    }

    #[must_use]
    pub fn expect_imm_borrow(self) -> Self {
        self.expect(ExpectedDeco::imm_borrow())
    }

    #[must_use]
    pub fn expect_imm_borrow_at(self, text: &str) -> Self {
        self.expect(ExpectedDeco::imm_borrow().at_text(text))
    }

    #[must_use]
    pub fn expect_mut_borrow(self) -> Self {
        self.expect(ExpectedDeco::mut_borrow())
    }

    #[must_use]
    pub fn expect_mut_borrow_at(self, text: &str) -> Self {
        self.expect(ExpectedDeco::mut_borrow().at_text(text))
    }

    #[must_use]
    pub fn expect_call(self) -> Self {
        self.expect(ExpectedDeco::call())
    }

    #[must_use]
    pub fn expect_call_at(self, text: &str) -> Self {
        self.expect(ExpectedDeco::call().at_text(text))
    }

    #[must_use]
    pub fn expect_lifetime(self) -> Self {
        self.expect(ExpectedDeco::lifetime())
    }

    #[must_use]
    pub fn expect_lifetime_at(self, text: &str) -> Self {
        self.expect(ExpectedDeco::lifetime().at_text(text))
    }

    #[must_use]
    pub fn expect_shared_mut(self) -> Self {
        self.expect(ExpectedDeco::shared_mut())
    }

    #[must_use]
    pub fn expect_outlive(self) -> Self {
        self.expect(ExpectedDeco::outlive())
    }

    #[must_use]
    pub fn forbid(mut self, kind: DecoKind) -> Self {
        self.forbidden_decos.push(kind);
        self
    }

    #[must_use]
    pub fn forbid_move(self) -> Self {
        self.forbid(DecoKind::Move)
    }

    #[must_use]
    pub fn forbid_outlive(self) -> Self {
        self.forbid(DecoKind::Outlive)
    }

    #[must_use]
    pub fn forbid_imm_borrow(self) -> Self {
        self.forbid(DecoKind::ImmBorrow)
    }

    #[must_use]
    pub fn forbid_mut_borrow(self) -> Self {
        self.forbid(DecoKind::MutBorrow)
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("TestCase serialization should not fail")
    }

    /// Run the test case by spawning the test-runner binary.
    ///
    /// # Panics
    /// Panics if the test fails or if the test-runner binary cannot be found.
    pub fn run(self) {
        let result = execute_test(self);
        assert!(
            result.passed,
            "Test '{}' failed:\n{}",
            result.name,
            result.error.unwrap_or_default()
        );
    }
}

fn dedent(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let first_non_empty = lines.iter().position(|l| !l.trim().is_empty());
    let last_non_empty = lines.iter().rposition(|l| !l.trim().is_empty());

    let (start, end) = match (first_non_empty, last_non_empty) {
        (Some(s), Some(e)) => (s, e),
        _ => return String::new(),
    };

    let trimmed_lines = &lines[start..=end];

    let min_indent = trimmed_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    trimmed_lines
        .iter()
        .map(|l| {
            if l.len() >= min_indent {
                &l[min_indent..]
            } else {
                l.trim_start()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

fn find_test_runner() -> PathBuf {
    if let Ok(path) = env::var("FERROUS_OWL_TEST_RUNNER") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return p;
        }
    }

    // Try to find relative to CARGO_MANIFEST_DIR (workspace root)
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        // From crates/owl-test, go up to workspace root
        let workspace_root = PathBuf::from(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(&manifest_dir));
        
        let target_dir = workspace_root.join("target");
        for profile in ["debug", "release"] {
            let runner = target_dir.join(profile).join("test-runner");
            if runner.exists() {
                return runner;
            }
        }
    }

    // Try relative to current exe
    if let Ok(exe) = env::current_exe() {
        if let Some(target_dir) = exe.parent() {
            let runner = target_dir.join("test-runner");
            if runner.exists() {
                return runner;
            }
            // Try parent (e.g., target/debug/deps -> target/debug)
            if let Some(parent) = target_dir.parent() {
                let runner = parent.join("test-runner");
                if runner.exists() {
                    return runner;
                }
            }
        }
    }

    panic!(
        "Could not find test-runner binary. Build it with: cargo build --bin test-runner\n\
         Or set FERROUS_OWL_TEST_RUNNER environment variable."
    );
}

fn execute_test(test: TestCase) -> TestResult {
    let runner = find_test_runner();
    let json = test.to_json();
    let name = test.name.clone();

    eprintln!("[owl-test] Running test '{}' with runner: {}", name, runner.display());

    let mut child = Command::new(&runner)
        .arg("--single")
        .arg(&json)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to spawn test-runner at {}: {e}", runner.display()));

    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(ref mut out) = child.stdout {
        out.read_to_string(&mut stdout).ok();
    }
    if let Some(ref mut err) = child.stderr {
        err.read_to_string(&mut stderr).ok();
    }

    let status = child.wait().expect("Failed to wait for test-runner");

    let passed = status.success();
    let error = if passed {
        None
    } else {
        Some(format!("stdout:\n{stdout}\nstderr:\n{stderr}"))
    };

    TestResult {
        name,
        passed,
        error,
        stdout,
        stderr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedent_basic() {
        let code = r#"
            fn test() {
                let x = 1;
            }
        "#;
        let result = dedent(code);
        assert_eq!(result, "fn test() {\n    let x = 1;\n}");
    }

    #[test]
    fn test_dedent_empty() {
        assert_eq!(dedent(""), "");
        assert_eq!(dedent("   \n   \n"), "");
    }

    #[test]
    fn test_dedent_no_indent() {
        let code = "fn test() {}";
        assert_eq!(dedent(code), "fn test() {}");
    }

    #[test]
    fn test_case_serialization() {
        let test = TestCase::new("test", "fn test() {}")
            .cursor_on("test")
            .expect_move();

        let json = test.to_json();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"cursor_text\":\"test\""));
    }
}
