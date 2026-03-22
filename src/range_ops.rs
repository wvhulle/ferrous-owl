use crate::models::{Function, MirDecl, MirStatement, MirTerminator, Range};

#[must_use]
pub fn is_super_range(r1: Range, r2: Range) -> bool {
    (r1.from() < r2.from() && r2.until() <= r1.until())
        || (r1.from() <= r2.from() && r2.until() < r1.until())
}

#[must_use]
pub fn common_range(r1: Range, r2: Range) -> Option<Range> {
    if r2.from() < r1.from() {
        return common_range(r2, r1);
    }
    if r1.until() < r2.from() {
        return None;
    }
    let from = r2.from();
    let until = r1.until().min(r2.until());
    Range::new(from, until)
}

#[must_use]
pub fn common_ranges(ranges: &[Range]) -> Vec<Range> {
    let mut common_ranges = Vec::new();
    for i in 0..ranges.len() {
        for j in i + 1..ranges.len() {
            if let Some(common) = common_range(ranges[i], ranges[j]) {
                common_ranges.push(common);
            }
        }
    }
    eliminated_ranges(common_ranges)
}

/// merge two ranges, result is superset of two ranges
#[must_use]
pub fn merge_ranges(r1: Range, r2: Range) -> Option<Range> {
    if common_range(r1, r2).is_some() || r1.until() == r2.from() || r2.until() == r1.from() {
        let from = r1.from().min(r2.from());
        let until = r1.until().max(r2.until());
        Range::new(from, until)
    } else {
        None
    }
}

/// eliminate common ranges and flatten ranges
#[must_use]
pub fn eliminated_ranges(mut ranges: Vec<Range>) -> Vec<Range> {
    let mut i = 0;
    'outer: while i < ranges.len() {
        let mut j = 0;
        while j < ranges.len() {
            if i != j
                && let Some(merged) = merge_ranges(ranges[i], ranges[j])
            {
                ranges[i] = merged;
                ranges.remove(j);
                continue 'outer;
            }
            j += 1;
        }
        i += 1;
    }
    ranges
}

#[must_use]
pub fn exclude_ranges(mut from: Vec<Range>, excludes: &[Range]) -> Vec<Range> {
    let mut i = 0;
    'outer: while i < from.len() {
        let mut j = 0;
        while j < excludes.len() {
            if let Some(common) = common_range(from[i], excludes[j]) {
                if let Some(r) = Range::new(from[i].from(), common.from() - 1) {
                    from.push(r);
                }
                if let Some(r) = Range::new(common.until() + 1, from[i].until()) {
                    from.push(r);
                }
                from.remove(i);
                continue 'outer;
            }
            j += 1;
        }
        i += 1;
    }
    eliminated_ranges(from)
}

pub trait MirVisitor {
    fn visit_func(&mut self, _func: &Function) {}
    fn visit_decl(&mut self, _decl: &MirDecl) {}
    fn visit_stmt(&mut self, _stmt: &MirStatement) {}
    fn visit_term(&mut self, _term: &MirTerminator) {}
}
pub fn mir_visit(func: &Function, visitor: &mut impl MirVisitor) {
    visitor.visit_func(func);
    for decl in &func.decls {
        visitor.visit_decl(decl);
    }
    for bb in &func.basic_blocks {
        for stmt in &bb.statements {
            visitor.visit_stmt(stmt);
        }
        if let Some(term) = &bb.terminator {
            visitor.visit_term(term);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        FnLocal, Loc, MirBasicBlock, MirDecl, MirRval, MirStatement, MirTerminator,
    };

    fn r(from: u32, until: u32) -> Range {
        Range::new(Loc::from(from), Loc::from(until)).unwrap()
    }

    // ── is_super_range ──────────────────────────────────────────────

    #[test]
    fn super_range_strict_both_sides() {
        assert!(is_super_range(r(0, 10), r(2, 8)));
    }

    #[test]
    fn super_range_left_equal() {
        assert!(is_super_range(r(0, 10), r(0, 8)));
    }

    #[test]
    fn super_range_right_equal() {
        assert!(is_super_range(r(0, 10), r(2, 10)));
    }

    #[test]
    fn not_super_range_when_equal() {
        assert!(!is_super_range(r(0, 10), r(0, 10)));
    }

    #[test]
    fn not_super_range_when_smaller() {
        assert!(!is_super_range(r(2, 8), r(0, 10)));
    }

    #[test]
    fn not_super_range_disjoint() {
        assert!(!is_super_range(r(0, 5), r(6, 10)));
    }

    // ── common_range ────────────────────────────────────────────────

    #[test]
    fn common_range_overlap() {
        let c = common_range(r(0, 10), r(5, 15)).unwrap();
        assert_eq!(c, r(5, 10));
    }

    #[test]
    fn common_range_symmetric() {
        assert_eq!(
            common_range(r(0, 10), r(5, 15)),
            common_range(r(5, 15), r(0, 10))
        );
    }

    #[test]
    fn common_range_contained() {
        let c = common_range(r(0, 20), r(5, 10)).unwrap();
        assert_eq!(c, r(5, 10));
    }

    #[test]
    fn common_range_touching_boundary() {
        // r1.until() == r2.from() — ranges share a single point
        let c = common_range(r(0, 5), r(5, 10));
        assert!(c.is_none() || c.unwrap() == r(5, 5));
    }

    #[test]
    fn common_range_disjoint() {
        assert!(common_range(r(0, 5), r(6, 10)).is_none());
    }

    // ── merge_ranges ────────────────────────────────────────────────

    #[test]
    fn merge_overlapping() {
        let m = merge_ranges(r(0, 10), r(5, 15)).unwrap();
        assert_eq!(m, r(0, 15));
    }

    #[test]
    fn merge_adjacent() {
        let m = merge_ranges(r(0, 5), r(5, 10)).unwrap();
        assert_eq!(m, r(0, 10));
    }

    #[test]
    fn merge_disjoint_fails() {
        assert!(merge_ranges(r(0, 5), r(7, 10)).is_none());
    }

    // ── eliminated_ranges ───────────────────────────────────────────

    #[test]
    fn eliminate_merges_overlapping() {
        let result = eliminated_ranges(vec![r(0, 5), r(3, 8), r(10, 15)]);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&r(0, 8)));
        assert!(result.contains(&r(10, 15)));
    }

    #[test]
    fn eliminate_single_range() {
        let result = eliminated_ranges(vec![r(0, 10)]);
        assert_eq!(result, vec![r(0, 10)]);
    }

    #[test]
    fn eliminate_empty() {
        let result = eliminated_ranges(vec![]);
        assert!(result.is_empty());
    }

    // ── common_ranges ───────────────────────────────────────────────

    #[test]
    fn common_ranges_pairwise() {
        let result = common_ranges(&[r(0, 10), r(5, 15), r(20, 30)]);
        assert_eq!(result, vec![r(5, 10)]);
    }

    #[test]
    fn common_ranges_none() {
        let result = common_ranges(&[r(0, 5), r(10, 15)]);
        assert!(result.is_empty());
    }

    // ── exclude_ranges ──────────────────────────────────────────────

    #[test]
    fn exclude_middle() {
        let result = exclude_ranges(vec![r(0, 20)], &[r(5, 10)]);
        assert!(result.contains(&r(0, 4)));
        assert!(result.contains(&r(11, 20)));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn exclude_nothing() {
        let result = exclude_ranges(vec![r(0, 10)], &[r(20, 30)]);
        assert_eq!(result, vec![r(0, 10)]);
    }

    #[test]
    fn exclude_everything() {
        let result = exclude_ranges(vec![r(0, 10)], &[r(0, 10)]);
        assert!(result.is_empty());
    }

    // ── mir_visit ───────────────────────────────────────────────────

    struct VisitCounter {
        funcs: u32,
        decls: u32,
        stmts: u32,
        terms: u32,
    }

    impl VisitCounter {
        fn new() -> Self {
            Self {
                funcs: 0,
                decls: 0,
                stmts: 0,
                terms: 0,
            }
        }
    }

    impl MirVisitor for VisitCounter {
        fn visit_func(&mut self, _func: &Function) {
            self.funcs += 1;
        }
        fn visit_decl(&mut self, _decl: &MirDecl) {
            self.decls += 1;
        }
        fn visit_stmt(&mut self, _stmt: &MirStatement) {
            self.stmts += 1;
        }
        fn visit_term(&mut self, _term: &MirTerminator) {
            self.terms += 1;
        }
    }

    fn sample_function() -> Function {
        let local = FnLocal::new(1, 0);
        let span = r(0, 10);
        Function {
            fn_id: 0,
            decls: vec![
                MirDecl::User {
                    local,
                    name: "x".into(),
                    span,
                    ty: "i32".into(),
                    lives: vec![span],
                    shared_borrow: vec![],
                    mutable_borrow: vec![],
                    drop: false,
                    drop_range: vec![],
                    must_live_at: vec![],
                },
                MirDecl::Other {
                    local: FnLocal::new(2, 0),
                    ty: "&i32".into(),
                    lives: vec![],
                    shared_borrow: vec![],
                    mutable_borrow: vec![],
                    drop: false,
                    drop_range: vec![],
                    must_live_at: vec![],
                },
            ],
            basic_blocks: vec![
                MirBasicBlock {
                    statements: vec![
                        MirStatement::StorageLive {
                            target_local: local,
                            range: r(0, 5),
                        },
                        MirStatement::Assign {
                            target_local: local,
                            range: r(0, 8),
                            rval: Some(MirRval::Borrow {
                                target_local: FnLocal::new(2, 0),
                                range: r(4, 8),
                                mutable: false,
                                outlive: None,
                            }),
                        },
                    ],
                    terminator: Some(MirTerminator::Call {
                        destination_local: local,
                        fn_span: r(10, 20),
                    }),
                },
                MirBasicBlock {
                    statements: vec![MirStatement::Assign {
                        target_local: local,
                        range: r(20, 30),
                        rval: Some(MirRval::Move {
                            target_local: FnLocal::new(2, 0),
                            range: r(22, 28),
                        }),
                    }],
                    terminator: Some(MirTerminator::Drop {
                        local,
                        range: r(30, 35),
                    }),
                },
            ],
        }
    }

    #[test]
    fn mir_visit_counts() {
        let func = sample_function();
        let mut counter = VisitCounter::new();
        mir_visit(&func, &mut counter);
        assert_eq!(counter.funcs, 1);
        assert_eq!(counter.decls, 2);
        assert_eq!(counter.stmts, 3); // 2 in bb0, 1 in bb1
        assert_eq!(counter.terms, 2); // 1 per bb
    }

    #[test]
    fn mir_visit_empty_function() {
        let func = Function {
            fn_id: 0,
            decls: vec![],
            basic_blocks: vec![],
        };
        let mut counter = VisitCounter::new();
        mir_visit(&func, &mut counter);
        assert_eq!(counter.funcs, 1);
        assert_eq!(counter.decls, 0);
        assert_eq!(counter.stmts, 0);
        assert_eq!(counter.terms, 0);
    }

    #[test]
    fn mir_visit_bb_without_terminator() {
        let func = Function {
            fn_id: 0,
            decls: vec![],
            basic_blocks: vec![MirBasicBlock {
                statements: vec![MirStatement::Other { range: r(0, 5) }],
                terminator: None,
            }],
        };
        let mut counter = VisitCounter::new();
        mir_visit(&func, &mut counter);
        assert_eq!(counter.stmts, 1);
        assert_eq!(counter.terms, 0);
    }
}
