use serde::Deserialize;
use serde_json::Value;

/// Decoration kind from ferrous-owl
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DecoKind {
    Move,
    Copy,
    ImmBorrow,
    MutBorrow,
    Drop,
    Call,
    SharedMut,
    #[allow(dead_code, reason = "May be used in future tests")]
    PartialMove,
}

impl DecoKind {
    /// Convert to the string format used in LSP diagnostics.
    #[must_use]
    pub const fn as_code(&self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Copy => "copy",
            Self::ImmBorrow => "imm-borrow",
            Self::MutBorrow => "mut-borrow",
            Self::Drop => "drop",
            Self::Call => "call",
            Self::SharedMut => "shared-mut",
            Self::PartialMove => "partial-move",
        }
    }
}

/// Test case definition that can be deserialized from JSON.
#[derive(Debug, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub source: String,
    #[allow(dead_code, reason = "Used for cursor positioning in future")]
    pub cursor_line: u32,
    #[allow(dead_code, reason = "Used for cursor positioning in future")]
    pub cursor_char: u32,
    pub expected: Vec<ExpectedDeco>,
}

/// Expected decoration with location.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedDeco {
    pub kind: DecoKind,
    pub line: u32,
    pub start_char: u32,
    pub end_char: u32,
}

/// Received diagnostic from LSP.
#[derive(Debug, Clone)]
pub struct ReceivedDiagnostic {
    pub kind: String,
    pub line: u32,
    pub start_char: u32,
    pub end_char: u32,
}

impl ReceivedDiagnostic {
    /// Parse from LSP diagnostic JSON.
    #[allow(clippy::cast_possible_truncation, reason = "LSP line numbers fit in u32")]
    pub fn from_lsp(value: &Value) -> Option<Self> {
        let code = value.get("code")?.as_str()?;
        let range = value.get("range")?;
        let start = range.get("start")?;
        let end = range.get("end")?;

        Some(Self {
            kind: code.to_string(),
            line: start.get("line")?.as_u64()? as u32,
            start_char: start.get("character")?.as_u64()? as u32,
            end_char: end.get("character")?.as_u64()? as u32,
        })
    }

    /// Check if this diagnostic matches an expected decoration.
    #[must_use]
    pub fn matches(&self, expected: &ExpectedDeco) -> bool {
        self.kind == expected.kind.as_code()
            && self.line == expected.line
            && self.start_char == expected.start_char
            && self.end_char == expected.end_char
    }
}
