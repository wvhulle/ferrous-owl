use std::{collections::HashSet, mem, path::PathBuf};

use tower_lsp::lsp_types;

use crate::{
    lsp_progress::AnalysisStatus,
    models::{FnLocal, Loc, MirDecl, MirRval, MirStatement, MirTerminator, Range},
    range_ops, text_conversion,
};

impl<R> Deco<R> {
    /// Returns whether this decoration should be shown as a diagnostic.
    /// Lifetime decorations are filtered out as they are too verbose.
    pub const fn should_show_as_diagnostic(&self) -> bool {
        !matches!(self, Self::Lifetime { .. })
    }

    /// Returns the diagnostic severity for this decoration type.
    /// Each type gets a distinct severity for better visual differentiation:
    /// - Outlive -> Error (red - critical ownership issues)
    /// - `SharedMut`, Move -> Warning (yellow/orange - ownership/aliasing)
    /// - `MutBorrow`, Call -> Information (blue - mutable access/calls)
    /// - `ImmBorrow`, Lifetime -> Hint (gray/dim - immutable borrow info)
    pub const fn diagnostic_severity(&self) -> lsp_types::DiagnosticSeverity {
        match self {
            Self::Outlive { .. } => lsp_types::DiagnosticSeverity::ERROR,
            Self::SharedMut { .. } | Self::Move { .. } => lsp_types::DiagnosticSeverity::WARNING,
            Self::MutBorrow { .. } | Self::Call { .. } => {
                lsp_types::DiagnosticSeverity::INFORMATION
            }
            Self::ImmBorrow { .. } | Self::Lifetime { .. } => lsp_types::DiagnosticSeverity::HINT,
        }
    }

    /// Returns the hover text for this decoration
    pub fn hover_text(&self) -> &str {
        match self {
            Self::Lifetime { hover_text, .. }
            | Self::ImmBorrow { hover_text, .. }
            | Self::MutBorrow { hover_text, .. }
            | Self::Move { hover_text, .. }
            | Self::Call { hover_text, .. }
            | Self::SharedMut { hover_text, .. }
            | Self::Outlive { hover_text, .. } => hover_text,
        }
    }

    /// Returns a diagnostic code for this decoration type
    pub fn diagnostic_code(&self) -> String {
        let pkg = env!("CARGO_PKG_NAME");
        match self {
            Self::Lifetime { .. } => format!("{pkg}:lifetime"),
            Self::ImmBorrow { .. } => format!("{pkg}:imm-borrow"),
            Self::MutBorrow { .. } => format!("{pkg}:mut-borrow"),
            Self::Move { .. } => format!("{pkg}:move"),
            Self::Call { .. } => format!("{pkg}:call"),
            Self::SharedMut { .. } => format!("{pkg}:shared-mut"),
            Self::Outlive { .. } => format!("{pkg}:outlive"),
        }
    }
}

impl Deco<lsp_types::Range> {
    /// Convert this decoration to an LSP diagnostic
    #[must_use]
    pub fn to_diagnostic(&self) -> lsp_types::Diagnostic {
        let range = match self {
            Self::Lifetime { range, .. }
            | Self::ImmBorrow { range, .. }
            | Self::MutBorrow { range, .. }
            | Self::Move { range, .. }
            | Self::Call { range, .. }
            | Self::SharedMut { range, .. }
            | Self::Outlive { range, .. } => *range,
        };

        lsp_types::Diagnostic {
            range,
            severity: Some(self.diagnostic_severity()),
            code: Some(lsp_types::NumberOrString::String(self.diagnostic_code())),
            code_description: None,
            source: Some(env!("CARGO_PKG_NAME").to_string()),
            message: self.hover_text().to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }
}

// TODO: Variable name should be checked?
// const ASYNC_MIR_VARS: [&str; 2] = ["_task_context", "__awaitee"];
const ASYNC_RESUME_TY: [&str; 2] = [
    "std::future::ResumeTy",
    "impl std::future::Future<Output = ()>",
];

#[derive(serde::Serialize, PartialEq, Eq, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Deco<R = Range> {
    Lifetime {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    ImmBorrow {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    MutBorrow {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    Move {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    Call {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    SharedMut {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
    Outlive {
        local: FnLocal,
        range: R,
        hover_text: String,
        overlapped: bool,
    },
}
impl Deco<Range> {
    fn convert_range(s: &str, range: Range) -> lsp_types::Range {
        let start = text_conversion::index_to_line_char(s, range.from());
        let end = text_conversion::index_to_line_char(s, range.until());
        lsp_types::Range {
            start: lsp_types::Position {
                line: start.0,
                character: start.1,
            },
            end: lsp_types::Position {
                line: end.0,
                character: end.1,
            },
        }
    }

    const fn range(&self) -> Range {
        match self {
            Self::Lifetime { range, .. }
            | Self::ImmBorrow { range, .. }
            | Self::MutBorrow { range, .. }
            | Self::Move { range, .. }
            | Self::Call { range, .. }
            | Self::SharedMut { range, .. }
            | Self::Outlive { range, .. } => *range,
        }
    }

    const fn range_and_overlapped(&self) -> (Range, bool) {
        match self {
            Self::Lifetime {
                range, overlapped, ..
            }
            | Self::ImmBorrow {
                range, overlapped, ..
            }
            | Self::MutBorrow {
                range, overlapped, ..
            }
            | Self::Move {
                range, overlapped, ..
            }
            | Self::Call {
                range, overlapped, ..
            }
            | Self::SharedMut {
                range, overlapped, ..
            }
            | Self::Outlive {
                range, overlapped, ..
            } => (*range, *overlapped),
        }
    }

    fn with_range(&self, new_range: Range, overlapped: bool) -> Self {
        match self {
            Self::Lifetime {
                local, hover_text, ..
            } => Self::Lifetime {
                local: *local,
                range: new_range,
                hover_text: hover_text.clone(),
                overlapped,
            },
            Self::ImmBorrow {
                local, hover_text, ..
            } => Self::ImmBorrow {
                local: *local,
                range: new_range,
                hover_text: hover_text.clone(),
                overlapped,
            },
            Self::MutBorrow {
                local, hover_text, ..
            } => Self::MutBorrow {
                local: *local,
                range: new_range,
                hover_text: hover_text.clone(),
                overlapped,
            },
            Self::Move {
                local, hover_text, ..
            } => Self::Move {
                local: *local,
                range: new_range,
                hover_text: hover_text.clone(),
                overlapped,
            },
            Self::Call {
                local, hover_text, ..
            } => Self::Call {
                local: *local,
                range: new_range,
                hover_text: hover_text.clone(),
                overlapped,
            },
            Self::SharedMut {
                local, hover_text, ..
            } => Self::SharedMut {
                local: *local,
                range: new_range,
                hover_text: hover_text.clone(),
                overlapped,
            },
            Self::Outlive {
                local, hover_text, ..
            } => Self::Outlive {
                local: *local,
                range: new_range,
                hover_text: hover_text.clone(),
                overlapped,
            },
        }
    }

    #[must_use]
    pub fn to_lsp_range(&self, s: &str) -> Deco<lsp_types::Range> {
        match self.clone() {
            Self::Lifetime {
                local,
                range,
                hover_text,
                overlapped,
            } => Deco::Lifetime {
                local,
                range: Self::convert_range(s, range),
                hover_text,
                overlapped,
            },
            Self::ImmBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => Deco::ImmBorrow {
                local,
                range: Self::convert_range(s, range),
                hover_text,
                overlapped,
            },
            Self::MutBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => Deco::MutBorrow {
                local,
                range: Self::convert_range(s, range),
                hover_text,
                overlapped,
            },
            Self::Move {
                local,
                range,
                hover_text,
                overlapped,
            } => Deco::Move {
                local,
                range: Self::convert_range(s, range),
                hover_text,
                overlapped,
            },
            Self::Call {
                local,
                range,
                hover_text,
                overlapped,
            } => Deco::Call {
                local,
                range: Self::convert_range(s, range),
                hover_text,
                overlapped,
            },
            Self::SharedMut {
                local,
                range,
                hover_text,
                overlapped,
            } => Deco::SharedMut {
                local,
                range: Self::convert_range(s, range),
                hover_text,
                overlapped,
            },
            Self::Outlive {
                local,
                range,
                hover_text,
                overlapped,
            } => Deco::Outlive {
                local,
                range: Self::convert_range(s, range),
                hover_text,
                overlapped,
            },
        }
    }
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct Decorations {
    pub is_analyzed: bool,
    pub status: AnalysisStatus,
    pub path: Option<PathBuf>,
    #[serde(rename = "decorations")]
    pub items: Vec<Deco<lsp_types::Range>>,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CursorRequest {
    pub position: lsp_types::Position,
    pub document: lsp_types::TextDocumentIdentifier,
}
impl CursorRequest {
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.document.uri.to_file_path().ok()
    }
    #[must_use]
    pub const fn position(&self) -> lsp_types::Position {
        self.position
    }
}

#[derive(Clone, Copy, Debug)]
enum SelectReason {
    Var,
    Move,
    Borrow,
    Call,
}
#[derive(Clone, Debug)]
pub struct SelectLocal {
    pos: Loc,
    candidate_local_decls: Vec<FnLocal>,
    selected: Option<(SelectReason, FnLocal, Range)>,
}
impl SelectLocal {
    #[must_use]
    pub const fn new(pos: Loc) -> Self {
        Self {
            pos,
            candidate_local_decls: Vec::new(),
            selected: None,
        }
    }

    fn select(&mut self, reason: SelectReason, local: FnLocal, range: Range) {
        if !self.candidate_local_decls.contains(&local) {
            return;
        }
        if range.from() <= self.pos && self.pos <= range.until() {
            if let Some((old_reason, _, old_range)) = self.selected {
                match (old_reason, reason) {
                    (_, SelectReason::Var) => {
                        if range.size() < old_range.size() {
                            self.selected = Some((reason, local, range));
                        }
                    }
                    (SelectReason::Var, _) => {}
                    (_, SelectReason::Move | SelectReason::Borrow) => {
                        if range.size() < old_range.size() {
                            self.selected = Some((reason, local, range));
                        }
                    }
                    (SelectReason::Call, SelectReason::Call) => {
                        // TODO: select narrower when callee is method
                        if old_range.size() < range.size() {
                            self.selected = Some((reason, local, range));
                        }
                    }
                    _ => {}
                }
            } else {
                self.selected = Some((reason, local, range));
            }
        }
    }

    #[must_use]
    pub fn selected(&self) -> Option<FnLocal> {
        self.selected.map(|v| v.1)
    }
}
impl range_ops::MirVisitor for SelectLocal {
    fn visit_decl(&mut self, decl: &MirDecl) {
        let (local, ty) = match decl {
            MirDecl::User { local, ty, .. } | MirDecl::Other { local, ty, .. } => (local, ty),
        };
        if ASYNC_RESUME_TY.contains(&ty.as_str()) {
            return;
        }
        self.candidate_local_decls.push(*local);
        if let MirDecl::User { local, span, .. } = decl {
            self.select(SelectReason::Var, *local, *span);
        }
    }
    fn visit_stmt(&mut self, stmt: &MirStatement) {
        if let MirStatement::Assign { rval, .. } = stmt {
            match rval {
                Some(MirRval::Move {
                    target_local,
                    range,
                }) => {
                    self.select(SelectReason::Move, *target_local, *range);
                }
                Some(MirRval::Borrow {
                    target_local,
                    range,
                    ..
                }) => {
                    self.select(SelectReason::Borrow, *target_local, *range);
                }
                _ => {}
            }
        }
    }
    fn visit_term(&mut self, term: &MirTerminator) {
        if let MirTerminator::Call {
            destination_local,
            fn_span,
        } = term
        {
            self.select(SelectReason::Call, *destination_local, *fn_span);
        }
    }
}
#[derive(Clone, Debug)]
pub struct CalcDecos {
    locals: HashSet<FnLocal>,
    decorations: Vec<Deco>,
    current_fn_id: u32,
}
impl CalcDecos {
    pub fn new(locals: impl IntoIterator<Item = FnLocal>) -> Self {
        Self {
            locals: locals.into_iter().collect(),
            decorations: Vec::new(),
            current_fn_id: 0,
        }
    }

    const fn get_deco_order(deco: &Deco) -> u8 {
        match deco {
            Deco::Lifetime { .. } => 0,
            Deco::ImmBorrow { .. } => 1,
            Deco::MutBorrow { .. } => 2,
            Deco::Move { .. } => 3,
            Deco::Call { .. } => 4,
            Deco::SharedMut { .. } => 5,
            Deco::Outlive { .. } => 6,
        }
    }

    fn sort_by_definition(&mut self) {
        self.decorations.sort_by_key(Self::get_deco_order);
    }

    fn process_overlap(prev: &Deco, current_range: Range) -> Option<(Deco, Vec<Deco>)> {
        let (prev_range, prev_overlapped) = prev.range_and_overlapped();
        if prev_overlapped {
            return None;
        }

        range_ops::common_range(current_range, prev_range).map(|common| {
            let non_overlapping = range_ops::exclude_ranges(vec![prev_range], &[common]);
            let new_decos: Vec<_> = non_overlapping
                .into_iter()
                .map(|range| prev.with_range(range, false))
                .collect();
            let overlapped_deco = prev.with_range(common, true);
            (overlapped_deco, new_decos)
        })
    }

    pub fn handle_overlapping(&mut self) {
        self.sort_by_definition();

        let mut result: Vec<Deco> = Vec::with_capacity(self.decorations.len());

        for current in mem::take(&mut self.decorations) {
            let current_range = current.range();

            if result.iter().any(|prev| prev == &current) {
                continue;
            }

            let mut insertions: Vec<(usize, Vec<Deco>)> = Vec::new();

            for (j, prev) in result.iter_mut().enumerate() {
                if let Some((overlapped_deco, new_decos)) =
                    Self::process_overlap(prev, current_range)
                {
                    *prev = overlapped_deco;
                    if !new_decos.is_empty() {
                        insertions.push((j + 1, new_decos));
                    }
                }
            }

            for (offset, (pos, decos)) in insertions.into_iter().enumerate() {
                let insert_pos = pos + offset * decos.len();
                for (k, deco) in decos.into_iter().enumerate() {
                    result.insert(insert_pos + k, deco);
                }
            }

            result.push(current);
        }

        self.decorations = result;
    }

    #[must_use]
    pub fn decorations(self) -> Vec<Deco> {
        self.decorations
    }
}
impl range_ops::MirVisitor for CalcDecos {
    fn visit_decl(&mut self, decl: &MirDecl) {
        let (local, lives, shared_borrow, mutable_borrow, drop_range, must_live_at, name, drop) =
            match decl {
                MirDecl::User {
                    local,
                    name,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    drop,
                    ..
                } => (
                    *local,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    Some(name),
                    drop,
                ),
                MirDecl::Other {
                    local,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    drop,
                    ..
                } => (
                    *local,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    must_live_at,
                    None,
                    drop,
                ),
            };
        self.current_fn_id = local.fn_id;
        if self.locals.contains(&local) {
            let var_str = name.map_or_else(
                || "anonymous variable".to_owned(),
                |mir_var_name| format!("variable `{mir_var_name}`"),
            );
            // merge Drop object lives
            let drop_copy_live = if *drop {
                range_ops::eliminated_ranges(drop_range.clone())
            } else {
                range_ops::eliminated_ranges(lives.clone())
            };
            for range in &drop_copy_live {
                self.decorations.push(Deco::Lifetime {
                    local,
                    range: *range,
                    hover_text: format!("lifetime of {var_str}"),
                    overlapped: false,
                });
            }
            let mut borrow_ranges = shared_borrow.clone();
            borrow_ranges.extend_from_slice(mutable_borrow);
            let shared_mut = range_ops::common_ranges(&borrow_ranges);
            for range in shared_mut {
                self.decorations.push(Deco::SharedMut {
                    local,
                    range,
                    hover_text: format!("immutable and mutable borrows of {var_str} exist here"),
                    overlapped: false,
                });
            }
            let outlive = range_ops::exclude_ranges(must_live_at.clone(), &drop_copy_live);
            for range in outlive {
                self.decorations.push(Deco::Outlive {
                    local,
                    range,
                    hover_text: format!("{var_str} is required to live here"),
                    overlapped: false,
                });
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &MirStatement) {
        if let MirStatement::Assign { rval, .. } = stmt {
            match rval {
                Some(MirRval::Move {
                    target_local,
                    range,
                }) => {
                    if self.locals.contains(target_local) {
                        self.decorations.push(Deco::Move {
                            local: *target_local,
                            range: *range,
                            hover_text: "variable moved".to_string(),
                            overlapped: false,
                        });
                    }
                }
                Some(MirRval::Borrow {
                    target_local,
                    range,
                    mutable,
                    ..
                }) => {
                    if self.locals.contains(target_local) {
                        if *mutable {
                            self.decorations.push(Deco::MutBorrow {
                                local: *target_local,
                                range: *range,
                                hover_text: "mutable borrow".to_string(),
                                overlapped: false,
                            });
                        } else {
                            self.decorations.push(Deco::ImmBorrow {
                                local: *target_local,
                                range: *range,
                                hover_text: "immutable borrow".to_string(),
                                overlapped: false,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_term(&mut self, term: &MirTerminator) {
        if let MirTerminator::Call {
            destination_local,
            fn_span,
        } = term
            && self.locals.contains(destination_local)
        {
            let mut i = 0;
            for deco in &self.decorations {
                if let Deco::Call { range, .. } = deco
                    && range_ops::is_super_range(*fn_span, *range)
                {
                    return;
                }
            }
            while i < self.decorations.len() {
                let range = match &self.decorations[i] {
                    Deco::Call { range, .. } => Some(range),
                    _ => None,
                };
                if let Some(range) = range
                    && range_ops::is_super_range(*range, *fn_span)
                {
                    self.decorations.remove(i);
                    continue;
                }
                i += 1;
            }
            self.decorations.push(Deco::Call {
                local: *destination_local,
                range: *fn_span,
                hover_text: "function call".to_string(),
                overlapped: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{
            FnLocal, Function, Loc, MirBasicBlock, MirDecl, MirRval, MirStatement, MirTerminator,
            Range,
        },
        range_ops::mir_visit,
    };

    fn r(from: u32, until: u32) -> Range {
        Range::new(Loc::from(from), Loc::from(until)).unwrap()
    }

    fn local(id: u32) -> FnLocal {
        FnLocal::new(id, 0)
    }

    fn user_decl(id: u32, name: &str, span: Range) -> MirDecl {
        MirDecl::User {
            local: local(id),
            name: name.into(),
            span,
            ty: "i32".into(),
            lives: vec![span],
            shared_borrow: vec![],
            mutable_borrow: vec![],
            drop: false,
            drop_range: vec![],
            must_live_at: vec![],
        }
    }

    // ── SelectLocal ─────────────────────────────────────────────────

    #[test]
    fn select_local_on_var_decl() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(5, 10))],
            basic_blocks: vec![],
        };
        let mut sel = SelectLocal::new(Loc::from(7u32));
        mir_visit(&func, &mut sel);
        assert_eq!(sel.selected(), Some(local(1)));
    }

    #[test]
    fn select_local_outside_span_returns_none() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(5, 10))],
            basic_blocks: vec![],
        };
        let mut sel = SelectLocal::new(Loc::from(20u32));
        mir_visit(&func, &mut sel);
        assert_eq!(sel.selected(), None);
    }

    #[test]
    fn select_local_on_move_rval() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 3))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(10, 20),
                    rval: Some(MirRval::Move {
                        target_local: local(1),
                        range: r(15, 18),
                    }),
                }],
                terminator: None,
            }],
        };
        let mut sel = SelectLocal::new(Loc::from(16u32));
        mir_visit(&func, &mut sel);
        assert_eq!(sel.selected(), Some(local(1)));
    }

    #[test]
    fn select_local_on_borrow_rval() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 3))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(10, 20),
                    rval: Some(MirRval::Borrow {
                        target_local: local(1),
                        range: r(12, 17),
                        mutable: true,
                        outlive: None,
                    }),
                }],
                terminator: None,
            }],
        };
        let mut sel = SelectLocal::new(Loc::from(14u32));
        mir_visit(&func, &mut sel);
        assert_eq!(sel.selected(), Some(local(1)));
    }

    #[test]
    fn select_local_on_call_terminator() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 3))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![],
                terminator: Some(MirTerminator::Call {
                    destination_local: local(1),
                    fn_span: r(30, 40),
                }),
            }],
        };
        let mut sel = SelectLocal::new(Loc::from(35u32));
        mir_visit(&func, &mut sel);
        assert_eq!(sel.selected(), Some(local(1)));
    }

    #[test]
    fn select_local_var_wins_over_move_at_same_pos() {
        // If cursor is inside both a var span and a move span,
        // var selection should take priority (smaller span wins first, but
        // once a Var is selected, non-Var reasons cannot override it).
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(5, 8))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(3, 20),
                    rval: Some(MirRval::Move {
                        target_local: local(1),
                        range: r(3, 20),
                    }),
                }],
                terminator: None,
            }],
        };
        let mut sel = SelectLocal::new(Loc::from(6u32));
        mir_visit(&func, &mut sel);
        assert_eq!(sel.selected(), Some(local(1)));
    }

    #[test]
    fn select_local_async_resume_ty_filtered() {
        let func = Function {
            fn_id: 0,
            decls: vec![MirDecl::Other {
                local: local(1),
                ty: "std::future::ResumeTy".into(),
                lives: vec![],
                shared_borrow: vec![],
                mutable_borrow: vec![],
                drop: false,
                drop_range: vec![],
                must_live_at: vec![],
            }],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(0, 10),
                    rval: Some(MirRval::Move {
                        target_local: local(1),
                        range: r(0, 10),
                    }),
                }],
                terminator: None,
            }],
        };
        let mut sel = SelectLocal::new(Loc::from(5u32));
        mir_visit(&func, &mut sel);
        // async ResumeTy locals are excluded from candidate_local_decls
        assert_eq!(sel.selected(), None);
    }

    // ── CalcDecos: basic decoration generation ──────────────────────

    fn func_with_move() -> Function {
        Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 5))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(10, 20),
                    rval: Some(MirRval::Move {
                        target_local: local(1),
                        range: r(12, 18),
                    }),
                }],
                terminator: None,
            }],
        }
    }

    #[test]
    fn calc_decos_move() {
        let func = func_with_move();
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos
                .iter()
                .any(|d| matches!(d, Deco::Move { range, .. } if *range == r(12, 18))),
            "Expected Move deco at [12,18], got: {decos:?}"
        );
    }

    #[test]
    fn calc_decos_imm_borrow() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 5))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(10, 20),
                    rval: Some(MirRval::Borrow {
                        target_local: local(1),
                        range: r(12, 18),
                        mutable: false,
                        outlive: None,
                    }),
                }],
                terminator: None,
            }],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos
                .iter()
                .any(|d| matches!(d, Deco::ImmBorrow { range, .. } if *range == r(12, 18))),
            "Expected ImmBorrow deco, got: {decos:?}"
        );
    }

    #[test]
    fn calc_decos_mut_borrow() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 5))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(10, 20),
                    rval: Some(MirRval::Borrow {
                        target_local: local(1),
                        range: r(12, 18),
                        mutable: true,
                        outlive: None,
                    }),
                }],
                terminator: None,
            }],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos
                .iter()
                .any(|d| matches!(d, Deco::MutBorrow { range, .. } if *range == r(12, 18))),
            "Expected MutBorrow deco, got: {decos:?}"
        );
    }

    #[test]
    fn calc_decos_call() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 5))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![],
                terminator: Some(MirTerminator::Call {
                    destination_local: local(1),
                    fn_span: r(30, 40),
                }),
            }],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos
                .iter()
                .any(|d| matches!(d, Deco::Call { range, .. } if *range == r(30, 40))),
            "Expected Call deco, got: {decos:?}"
        );
    }

    #[test]
    fn calc_decos_lifetime_from_user_decl() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 20))],
            basic_blocks: vec![],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos
                .iter()
                .any(|d| matches!(d, Deco::Lifetime { range, hover_text, .. }
                if *range == r(0, 20) && hover_text.contains("lifetime"))),
            "Expected Lifetime deco, got: {decos:?}"
        );
    }

    #[test]
    fn calc_decos_ignores_unselected_local() {
        let func = func_with_move();
        let mut calc = CalcDecos::new([local(99)]); // different local
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos.is_empty(),
            "Expected no decos for unrelated local, got: {decos:?}"
        );
    }

    // ── CalcDecos: nested call deduplication ────────────────────────

    #[test]
    fn calc_decos_wider_call_skipped_when_narrow_exists() {
        // When a narrow Call exists first, then a wider one comes, the wider is skipped
        // because is_super_range(new_fn_span, existing_range) returns true → early
        // return
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 5))],
            basic_blocks: vec![
                MirBasicBlock {
                    statements: vec![],
                    terminator: Some(MirTerminator::Call {
                        destination_local: local(1),
                        fn_span: r(15, 40),
                    }),
                },
                MirBasicBlock {
                    statements: vec![],
                    terminator: Some(MirTerminator::Call {
                        destination_local: local(1),
                        fn_span: r(10, 50),
                    }),
                },
            ],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        let calls: Vec<_> = decos
            .iter()
            .filter(|d| matches!(d, Deco::Call { .. }))
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "Wider call should be skipped, got: {calls:?}"
        );
        assert!(matches!(calls[0], Deco::Call { range, .. } if *range == r(15, 40)));
    }

    // ── CalcDecos: SharedMut from overlapping borrow ranges ─────────

    #[test]
    fn calc_decos_shared_mut() {
        let func = Function {
            fn_id: 0,
            decls: vec![MirDecl::User {
                local: local(1),
                name: "x".into(),
                span: r(0, 5),
                ty: "i32".into(),
                lives: vec![r(0, 50)],
                shared_borrow: vec![r(10, 30)],
                mutable_borrow: vec![r(20, 40)],
                drop: false,
                drop_range: vec![],
                must_live_at: vec![],
            }],
            basic_blocks: vec![],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos
                .iter()
                .any(|d| matches!(d, Deco::SharedMut { range, .. } if *range == r(20, 30))),
            "Expected SharedMut deco at [20,30], got: {decos:?}"
        );
    }

    // ── CalcDecos: Outlive decorations ──────────────────────────────

    #[test]
    fn calc_decos_outlive() {
        let func = Function {
            fn_id: 0,
            decls: vec![MirDecl::User {
                local: local(1),
                name: "x".into(),
                span: r(0, 5),
                ty: "i32".into(),
                lives: vec![r(0, 20)],
                shared_borrow: vec![],
                mutable_borrow: vec![],
                drop: false,
                drop_range: vec![],
                must_live_at: vec![r(0, 30)],
            }],
            basic_blocks: vec![],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        let decos = calc.decorations();
        assert!(
            decos
                .iter()
                .any(|d| matches!(d, Deco::Outlive { range, .. } if *range == r(21, 30))),
            "Expected Outlive deco for range beyond lifetime, got: {decos:?}"
        );
    }

    // ── handle_overlapping ──────────────────────────────────────────

    #[test]
    fn handle_overlapping_splits_lifetime_around_move() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 30))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(10, 20),
                    rval: Some(MirRval::Move {
                        target_local: local(1),
                        range: r(12, 18),
                    }),
                }],
                terminator: None,
            }],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        calc.handle_overlapping();
        let decos = calc.decorations();

        // The lifetime [0,30] should be split around the move [12,18]:
        // non-overlapped: [0,11] and [19,30], overlapped: [12,18]
        let lifetimes: Vec<_> = decos
            .iter()
            .filter(|d| matches!(d, Deco::Lifetime { .. }))
            .collect();
        assert!(
            lifetimes.len() >= 2,
            "Lifetime should be split into at least 2 parts, got: {lifetimes:?}"
        );
        let overlapped_count = lifetimes
            .iter()
            .filter(|d| {
                matches!(
                    d,
                    Deco::Lifetime {
                        overlapped: true,
                        ..
                    }
                )
            })
            .count();
        assert!(
            overlapped_count >= 1,
            "At least one lifetime part should be marked overlapped"
        );
    }

    #[test]
    fn handle_overlapping_no_overlap_unchanged() {
        let func = Function {
            fn_id: 0,
            decls: vec![user_decl(1, "x", r(0, 10))],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Assign {
                    target_local: local(1),
                    range: r(50, 60),
                    rval: Some(MirRval::Move {
                        target_local: local(1),
                        range: r(52, 58),
                    }),
                }],
                terminator: None,
            }],
        };
        let mut calc = CalcDecos::new([local(1)]);
        mir_visit(&func, &mut calc);
        calc.handle_overlapping();
        let decos = calc.decorations();
        let overlapped_count = decos
            .iter()
            .filter(|d| match d {
                Deco::Lifetime { overlapped, .. } | Deco::Move { overlapped, .. } => *overlapped,
                _ => false,
            })
            .count();
        assert_eq!(overlapped_count, 0, "No overlaps expected, got: {decos:?}");
    }

    // ── Deco methods ────────────────────────────────────────────────

    #[test]
    fn should_show_as_diagnostic_filters_lifetime() {
        let lifetime = Deco::Lifetime::<Range> {
            local: local(1),
            range: r(0, 10),
            hover_text: "test".into(),
            overlapped: false,
        };
        assert!(!lifetime.should_show_as_diagnostic());

        let mv = Deco::Move::<Range> {
            local: local(1),
            range: r(0, 10),
            hover_text: "test".into(),
            overlapped: false,
        };
        assert!(mv.should_show_as_diagnostic());
    }

    #[test]
    fn diagnostic_severity_mapping() {
        let outlive = Deco::Outlive::<Range> {
            local: local(1),
            range: r(0, 1),
            hover_text: String::new(),
            overlapped: false,
        };
        assert_eq!(
            outlive.diagnostic_severity(),
            lsp_types::DiagnosticSeverity::ERROR
        );

        let mv = Deco::Move::<Range> {
            local: local(1),
            range: r(0, 1),
            hover_text: String::new(),
            overlapped: false,
        };
        assert_eq!(
            mv.diagnostic_severity(),
            lsp_types::DiagnosticSeverity::WARNING
        );

        let call = Deco::Call::<Range> {
            local: local(1),
            range: r(0, 1),
            hover_text: String::new(),
            overlapped: false,
        };
        assert_eq!(
            call.diagnostic_severity(),
            lsp_types::DiagnosticSeverity::INFORMATION
        );

        let imm = Deco::ImmBorrow::<Range> {
            local: local(1),
            range: r(0, 1),
            hover_text: String::new(),
            overlapped: false,
        };
        assert_eq!(
            imm.diagnostic_severity(),
            lsp_types::DiagnosticSeverity::HINT
        );
    }

    #[test]
    fn diagnostic_code_contains_pkg_name() {
        let deco = Deco::Move::<Range> {
            local: local(1),
            range: r(0, 10),
            hover_text: "test".into(),
            overlapped: false,
        };
        let code = deco.diagnostic_code();
        assert!(
            code.contains("ferrous-owl"),
            "Code should contain package name, got: {code}"
        );
        assert!(
            code.contains("move"),
            "Code should contain deco type, got: {code}"
        );
    }

    #[test]
    fn all_deco_variants_should_show_as_diagnostic() {
        let l = local(1);
        let range = r(0, 1);
        let ht = String::new();

        // Lifetime is the only one that should NOT show as diagnostic
        assert!(
            !Deco::Lifetime::<Range> {
                local: l,
                range,
                hover_text: ht.clone(),
                overlapped: false
            }
            .should_show_as_diagnostic()
        );
        assert!(
            Deco::ImmBorrow::<Range> {
                local: l,
                range,
                hover_text: ht.clone(),
                overlapped: false
            }
            .should_show_as_diagnostic()
        );
        assert!(
            Deco::MutBorrow::<Range> {
                local: l,
                range,
                hover_text: ht.clone(),
                overlapped: false
            }
            .should_show_as_diagnostic()
        );
        assert!(
            Deco::Move::<Range> {
                local: l,
                range,
                hover_text: ht.clone(),
                overlapped: false
            }
            .should_show_as_diagnostic()
        );
        assert!(
            Deco::Call::<Range> {
                local: l,
                range,
                hover_text: ht.clone(),
                overlapped: false
            }
            .should_show_as_diagnostic()
        );
        assert!(
            Deco::SharedMut::<Range> {
                local: l,
                range,
                hover_text: ht.clone(),
                overlapped: false
            }
            .should_show_as_diagnostic()
        );
        assert!(
            Deco::Outlive::<Range> {
                local: l,
                range,
                hover_text: ht,
                overlapped: false
            }
            .should_show_as_diagnostic()
        );
    }
}
