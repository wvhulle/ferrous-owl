use core::fmt::Display;
use std::{
    collections::HashMap,
    fmt,
    ops::{Add, Sub},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FnLocal {
    pub id: u32,
    pub fn_id: u32,
}

impl FnLocal {
    #[must_use]
    pub const fn new(id: u32, fn_id: u32) -> Self {
        Self { id, fn_id }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct Loc(u32);

impl Loc {
    #[must_use]
    pub fn from_byte_pos(source: &str, byte_pos: u32, offset: u32) -> Self {
        let byte_pos = byte_pos.saturating_sub(offset);
        // it seems that the compiler is ignoring CR
        let source_clean = source.replace('\r', "");

        // Convert byte position to character position safely
        if source_clean.len() < byte_pos as usize {
            return Self::from(source_clean.chars().count());
        }

        // Find the character index corresponding to the byte position

        source_clean
            .char_indices()
            .position(|(byte_idx, _)| (byte_pos as usize) <= byte_idx)
            .map_or_else(
                || {
                    #[allow(
                        clippy::cast_possible_truncation,
                        reason = "count is bounded by string length"
                    )]
                    Self(source_clean.chars().count() as u32)
                },
                Self::from,
            )
    }
}

impl Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add<i32> for Loc {
    type Output = Self;
    fn add(self, rhs: i32) -> Self::Output {
        Self(self.0.saturating_add_signed(rhs))
    }
}

impl Sub<i32> for Loc {
    type Output = Self;
    fn sub(self, rhs: i32) -> Self::Output {
        Self(self.0.saturating_add_signed(-rhs))
    }
}

impl From<u32> for Loc {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<u64> for Loc {
    fn from(value: u64) -> Self {
        Self(u32::try_from(value).unwrap_or(u32::MAX))
    }
}

impl From<usize> for Loc {
    fn from(value: usize) -> Self {
        Self(u32::try_from(value).unwrap_or(u32::MAX))
    }
}

impl From<Loc> for u32 {
    fn from(value: Loc) -> Self {
        value.0
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Range {
    from: Loc,
    until: Loc,
}

impl Range {
    #[must_use]
    pub const fn new(from: Loc, until: Loc) -> Option<Self> {
        if until.0 <= from.0 {
            None
        } else {
            Some(Self { from, until })
        }
    }
    #[must_use]
    pub const fn from(self) -> Loc {
        self.from
    }
    #[must_use]
    pub const fn until(self) -> Loc {
        self.until
    }
    #[must_use]
    pub const fn size(self) -> u32 {
        self.until.0 - self.from.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct File {
    pub items: Vec<Function>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct Workspace(pub HashMap<String, Crate>);

impl Workspace {
    #[cfg(test)]
    pub fn merge(&mut self, other: Self) {
        let Self(crates) = other;
        for (name, krate) in crates {
            if let Some(insert) = self.0.get_mut(&name) {
                insert.merge(krate);
            } else {
                self.0.insert(name, krate);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct Crate(pub HashMap<String, File>);

impl Crate {
    pub fn merge(&mut self, other: Self) {
        let Self(files) = other;
        for (file, mir) in files {
            if let Some(insert) = self.0.get_mut(&file) {
                insert.items.extend_from_slice(&mir.items);
                insert.items.dedup_by(|a, b| a.fn_id == b.fn_id);
            } else {
                self.0.insert(file, mir);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirRval {
    Move {
        target_local: FnLocal,
        range: Range,
    },
    Borrow {
        target_local: FnLocal,
        range: Range,
        mutable: bool,
        outlive: Option<Range>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirStatement {
    StorageLive {
        target_local: FnLocal,
        range: Range,
    },
    StorageDead {
        target_local: FnLocal,
        range: Range,
    },
    Assign {
        target_local: FnLocal,
        range: Range,
        rval: Option<MirRval>,
    },
    Other {
        range: Range,
    },
}
impl MirStatement {
    #[must_use]
    pub const fn range(&self) -> Range {
        match self {
            Self::StorageLive { range, .. }
            | Self::StorageDead { range, .. }
            | Self::Assign { range, .. }
            | Self::Other { range } => *range,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirTerminator {
    Drop {
        local: FnLocal,
        range: Range,
    },
    Call {
        destination_local: FnLocal,
        fn_span: Range,
    },
    Other {
        range: Range,
    },
}
impl MirTerminator {
    #[must_use]
    pub const fn range(&self) -> Range {
        match self {
            Self::Call { fn_span, .. } => *fn_span,
            Self::Drop { range, .. } | Self::Other { range } => *range,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirBasicBlock {
    pub statements: Vec<MirStatement>,
    pub terminator: Option<MirTerminator>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MirDecl {
    User {
        local: FnLocal,
        name: String,
        span: Range,
        ty: String,
        lives: Vec<Range>,
        shared_borrow: Vec<Range>,
        mutable_borrow: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        must_live_at: Vec<Range>,
    },
    Other {
        local: FnLocal,
        ty: String,
        lives: Vec<Range>,
        shared_borrow: Vec<Range>,
        mutable_borrow: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        must_live_at: Vec<Range>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Function {
    pub fn_id: u32,
    pub basic_blocks: Vec<MirBasicBlock>,
    pub decls: Vec<MirDecl>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loc_arithmetic_memory_safety() {
        let loc = Loc::from_byte_pos("test string with unicode 🦀", 5, 0);
        let loc2 = loc + 2;
        let loc3 = loc2 - 1;

        assert_eq!(loc3.0, loc.0 + 1);

        let loc_zero = Loc(0);
        let loc_underflow = loc_zero - 10;
        assert_eq!(loc_underflow.0, 0);

        let loc_large = Loc(u32::MAX - 10);
        let loc_add = loc_large + 5;
        assert_eq!(loc_add.0, u32::MAX - 5);
    }

    #[test]
    fn test_range_creation_and_validation() {
        let valid_range = Range::new(Loc(0), Loc(10)).unwrap();
        assert_eq!(valid_range.from().0, 0);
        assert_eq!(valid_range.until().0, 10);
        assert_eq!(valid_range.size(), 10);

        let invalid_range = Range::new(Loc(10), Loc(5));
        assert!(invalid_range.is_none());

        let same_pos_range = Range::new(Loc(5), Loc(5));
        assert!(same_pos_range.is_none());

        let large_range = Range::new(Loc(0), Loc(u32::MAX)).unwrap();
        assert_eq!(large_range.size(), u32::MAX);
    }

    #[test]
    fn test_fn_local_operations() {
        let fn_local1 = FnLocal::new(42, 100);
        let fn_local2 = FnLocal::new(43, 100);
        let fn_local3 = FnLocal::new(42, 100);

        assert_eq!(fn_local1, fn_local3);
        assert_ne!(fn_local1, fn_local2);

        let mut map = HashMap::new();
        map.insert(fn_local1, "first");
        map.insert(fn_local2, "second");
        map.insert(fn_local3, "third");

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&fn_local1), Some(&"third"));
        assert_eq!(map.get(&fn_local2), Some(&"second"));
    }

    #[test]
    fn test_file_model_operations() {
        let mut file = File { items: Vec::new() };

        assert_eq!(file.items.len(), 0);
        assert!(file.items.is_empty());

        file.items.reserve(1000);
        assert!(file.items.capacity() >= 1000);

        let file_clone = file.clone();
        assert_eq!(file.items.len(), file_clone.items.len());
    }

    #[test]
    fn test_workspace_operations() {
        let mut workspace = Workspace(HashMap::new());
        let mut crate1 = Crate(HashMap::new());
        let mut crate2 = Crate(HashMap::new());

        crate1
            .0
            .insert("lib.rs".to_string(), File { items: Vec::new() });
        crate1
            .0
            .insert("main.rs".to_string(), File { items: Vec::new() });

        crate2
            .0
            .insert("helper.rs".to_string(), File { items: Vec::new() });

        workspace.0.insert("crate1".to_string(), crate1);
        workspace.0.insert("crate2".to_string(), crate2);

        assert_eq!(workspace.0.len(), 2);
        assert!(workspace.0.contains_key("crate1"));
        assert!(workspace.0.contains_key("crate2"));

        let mut other_workspace = Workspace(HashMap::new());
        let crate3 = Crate(HashMap::new());
        other_workspace.0.insert("crate3".to_string(), crate3);

        workspace.merge(other_workspace);
        assert_eq!(workspace.0.len(), 3);
        assert!(workspace.0.contains_key("crate3"));
    }

    #[test]
    fn test_function_model_complex_operations() {
        let function = Function {
            fn_id: 42,
            basic_blocks: Vec::new(),
            decls: Vec::new(),
        };

        let function_clone = function.clone();
        assert_eq!(function.fn_id, function_clone.fn_id);
        assert_eq!(
            function.basic_blocks.len(),
            function_clone.basic_blocks.len()
        );
        assert_eq!(function.decls.len(), function_clone.decls.len());

        let function_size = size_of::<Function>();
        assert!(function_size > 0);

        let mut functions = Vec::new();
        for i in 0..100 {
            functions.push(Function {
                fn_id: i,
                basic_blocks: Vec::new(),
                decls: Vec::new(),
            });
        }

        assert_eq!(functions.len(), 100);
        assert_eq!(functions[50].fn_id, 50);

        let large_function = Function {
            fn_id: 999,
            basic_blocks: Vec::with_capacity(1000),
            decls: Vec::with_capacity(500),
        };

        assert!(large_function.basic_blocks.capacity() >= 1000);
        assert!(large_function.decls.capacity() >= 500);
    }

    #[test]
    fn test_string_handling_memory_safety() {
        let mut strings = Vec::new();

        for i in 0..50 {
            let s = format!("test_string_{i}");
            strings.push(s);
        }

        let mut concatenated = String::new();
        for s in &strings {
            concatenated.push_str(s);
            concatenated.push(' ');
        }

        assert!(!concatenated.is_empty());

        let unicode_string = "🦀 Rust 🔥 Memory Safety 🛡️".to_string();
        let _file = File { items: Vec::new() };

        assert!(unicode_string.len() > unicode_string.chars().count());
    }

    #[test]
    fn test_collections_memory_safety() {
        let mut map: HashMap<String, Vec<FnLocal>> = HashMap::new();

        for i in 0..20 {
            let key = format!("key_{i}");
            let mut vec = Vec::new();

            for j in 0..5 {
                vec.push(FnLocal::new(j, i));
            }

            map.insert(key, vec);
        }

        assert_eq!(map.len(), 20);

        for (key, vec) in &map {
            assert!(key.starts_with("key_"));
            assert_eq!(vec.len(), 5);

            for fn_local in vec {
                assert!(fn_local.id < 5);
                assert!(fn_local.fn_id < 20);
            }
        }

        let mut keys_to_remove = Vec::new();
        for key in map.keys() {
            if key.ends_with("_1") || key.ends_with("_2") {
                keys_to_remove.push(key.clone());
            }
        }

        for key in keys_to_remove {
            map.remove(&key);
        }

        assert_eq!(map.len(), 18);
    }

    #[test]
    fn test_serialization_structures() {
        let range = Range::new(Loc(10), Loc(20)).unwrap();
        let fn_local = FnLocal::new(1, 2);

        let range_clone = range;
        let fn_local_clone = fn_local;

        assert_eq!(range, range_clone);
        assert_eq!(fn_local, fn_local_clone);

        let debug_string = format!("{range:?}");
        assert!(debug_string.contains("Range"));

        let debug_fn_local = format!("{fn_local:?}");
        assert!(debug_fn_local.contains("FnLocal"));
    }
}
