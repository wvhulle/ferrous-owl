use std::{collections::HashSet, path::PathBuf};

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
    #[allow(
        clippy::too_many_lines,
        reason = "range conversion logic requires detailed matching"
    )]
    #[must_use]
    pub fn to_lsp_range(&self, s: &str) -> Deco<lsp_types::Range> {
        match self.clone() {
            Self::Lifetime {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = text_conversion::index_to_line_char(s, range.from());
                let end = text_conversion::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Lifetime {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }
            Self::ImmBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = text_conversion::index_to_line_char(s, range.from());
                let end = text_conversion::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::ImmBorrow {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }
            Self::MutBorrow {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = text_conversion::index_to_line_char(s, range.from());
                let end = text_conversion::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::MutBorrow {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }
            Self::Move {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = text_conversion::index_to_line_char(s, range.from());
                let end = text_conversion::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Move {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }
            Self::Call {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = text_conversion::index_to_line_char(s, range.from());
                let end = text_conversion::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Call {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }
            Self::SharedMut {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = text_conversion::index_to_line_char(s, range.from());
                let end = text_conversion::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::SharedMut {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }

            Self::Outlive {
                local,
                range,
                hover_text,
                overlapped,
            } => {
                let start = text_conversion::index_to_line_char(s, range.from());
                let end = text_conversion::index_to_line_char(s, range.until());
                let start = lsp_types::Position {
                    line: start.0,
                    character: start.1,
                };
                let end = lsp_types::Position {
                    line: end.0,
                    character: end.1,
                };
                Deco::Outlive {
                    local,
                    range: lsp_types::Range { start, end },
                    hover_text,
                    overlapped,
                }
            }
        }
    }
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct Decorations {
    pub is_analyzed: bool,
    pub status: AnalysisStatus,
    pub path: Option<PathBuf>,
    #[allow(
        clippy::struct_field_names,
        reason = "struct represents a collection of decorations"
    )]
    pub decorations: Vec<Deco<lsp_types::Range>>,
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

    #[allow(clippy::too_many_lines, reason = "complex overlap handling logic")]
    pub fn handle_overlapping(&mut self) {
        self.sort_by_definition();
        let mut i = 1;
        'outer: while i < self.decorations.len() {
            let current_range = match &self.decorations[i] {
                Deco::Lifetime { range, .. }
                | Deco::ImmBorrow { range, .. }
                | Deco::MutBorrow { range, .. }
                | Deco::Move { range, .. }
                | Deco::Call { range, .. }
                | Deco::SharedMut { range, .. }
                | Deco::Outlive { range, .. } => *range,
            };

            let mut j = 0;
            while j < i {
                let prev = &self.decorations[j];
                if prev == &self.decorations[i] {
                    self.decorations.remove(i);
                    continue 'outer;
                }
                let (prev_range, prev_overlapped) = match prev {
                    Deco::Lifetime {
                        range, overlapped, ..
                    }
                    | Deco::ImmBorrow {
                        range, overlapped, ..
                    }
                    | Deco::MutBorrow {
                        range, overlapped, ..
                    }
                    | Deco::Move {
                        range, overlapped, ..
                    }
                    | Deco::Call {
                        range, overlapped, ..
                    }
                    | Deco::SharedMut {
                        range, overlapped, ..
                    }
                    | Deco::Outlive {
                        range, overlapped, ..
                    } => (*range, *overlapped),
                };

                if prev_overlapped {
                    j += 1;
                    continue;
                }

                if let Some(common) = range_ops::common_range(current_range, prev_range) {
                    let mut new_decos = Vec::new();
                    let non_overlapping = range_ops::exclude_ranges(vec![prev_range], &[common]);

                    for range in non_overlapping {
                        let new_deco = match prev {
                            Deco::Lifetime {
                                local, hover_text, ..
                            } => Deco::Lifetime {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::ImmBorrow {
                                local, hover_text, ..
                            } => Deco::ImmBorrow {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::MutBorrow {
                                local, hover_text, ..
                            } => Deco::MutBorrow {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::Move {
                                local, hover_text, ..
                            } => Deco::Move {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::Call {
                                local, hover_text, ..
                            } => Deco::Call {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::SharedMut {
                                local, hover_text, ..
                            } => Deco::SharedMut {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                            Deco::Outlive {
                                local, hover_text, ..
                            } => Deco::Outlive {
                                local: *local,
                                range,
                                hover_text: hover_text.clone(),
                                overlapped: false,
                            },
                        };
                        new_decos.push(new_deco);
                    }

                    match &mut self.decorations[j] {
                        Deco::Lifetime {
                            range, overlapped, ..
                        }
                        | Deco::ImmBorrow {
                            range, overlapped, ..
                        }
                        | Deco::MutBorrow {
                            range, overlapped, ..
                        }
                        | Deco::Move {
                            range, overlapped, ..
                        }
                        | Deco::Call {
                            range, overlapped, ..
                        }
                        | Deco::SharedMut {
                            range, overlapped, ..
                        }
                        | Deco::Outlive {
                            range, overlapped, ..
                        } => {
                            *range = common;
                            *overlapped = true;
                        }
                    }

                    for (jj, deco) in new_decos.into_iter().enumerate() {
                        self.decorations.insert(j + jj + 1, deco);
                    }
                }
                j += 1;
            }
            i += 1;
        }
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

// TODO: new test
