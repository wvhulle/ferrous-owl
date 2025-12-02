use std::{collections::HashMap, env::current_dir, fs::read_to_string, future::Future, pin::Pin};

use rustc_borrowck::consumers::{
    ConsumerOptions, PoloniusInput, PoloniusOutput, get_body_with_borrowck_facts,
};
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_middle::{
    mir::{BasicBlock, Local},
    ty::TyCtxt,
};
use rustc_span::Span;

use crate::{
    mir_cache, mir_polonius, mir_transform,
    models::{FnLocal, Function, Loc, MirBasicBlock, MirDecl, Range},
};

pub type MirAnalyzeFuture = Pin<Box<dyn Future<Output = MirAnalyzer> + Send + Sync>>;

#[derive(Clone, Debug)]
pub struct AnalyzeResult {
    pub file_name: String,
    pub file_hash: String,
    pub mir_hash: String,
    pub analyzed: Function,
}

pub enum MirAnalyzerInitResult {
    Cached(AnalyzeResult),
    Analyzer(MirAnalyzeFuture),
}

pub fn range_from_span(source: &str, span: Span, offset: u32) -> Option<Range> {
    let from = Loc::from_byte_pos(source, span.lo().0, offset);
    let until = Loc::from_byte_pos(source, span.hi().0, offset);
    Range::new(from, until)
}

pub fn sort_locs(v: &mut [(BasicBlock, usize)]) {
    v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
}

pub struct MirAnalyzer {
    file_name: String,
    local_decls: HashMap<Local, String>,
    user_vars: HashMap<Local, (Range, String)>,
    input: PoloniusInput,
    basic_blocks: Vec<MirBasicBlock>,
    fn_id: LocalDefId,
    file_hash: String,
    mir_hash: String,
    accurate_live: HashMap<Local, Vec<Range>>,
    must_live: HashMap<Local, Vec<Range>>,
    shared_live: HashMap<Local, Vec<Range>>,
    mutable_live: HashMap<Local, Vec<Range>>,
    drop_range: HashMap<Local, Vec<Range>>,
}

impl MirAnalyzer {
    pub fn init(tcx: TyCtxt<'_>, fn_id: LocalDefId) -> MirAnalyzerInitResult {
        let mut facts =
            get_body_with_borrowck_facts(tcx, fn_id, ConsumerOptions::PoloniusInputFacts);
        let input = *facts.input_facts.take().unwrap();
        let location_table = facts.location_table.take().unwrap();

        let source_map = tcx.sess.source_map();

        let file_name = source_map.span_to_filename(facts.body.span);
        let source_file = source_map.get_source_file(&file_name).unwrap();
        let offset = source_file.start_pos.0;
        let file_name = source_map.path_mapping().to_embeddable_absolute_path(
            rustc_span::RealFileName::LocalPath(file_name.into_local_path().unwrap()),
            &rustc_span::RealFileName::LocalPath(current_dir().unwrap()),
        );
        let path = file_name.to_path(rustc_span::FileNameDisplayPreference::Local);
        let source = read_to_string(path).unwrap();
        let file_name = path.to_string_lossy().to_string();
        log::debug!("facts of {fn_id:?} prepared; start analyze of {fn_id:?}");

        let local_decls = facts
            .body
            .local_decls
            .iter_enumerated()
            .map(|(local, decl)| (local, decl.ty.to_string()))
            .collect();

        let mir_hash = mir_cache::Hasher::get_hash(
            tcx,
            mir_transform::erase_region_variables(tcx, facts.body.clone()),
        );
        let file_hash = mir_cache::Hasher::get_hash(tcx, &source);
        let mut cache = mir_cache::CACHE.lock().unwrap();

        if cache.is_none() {
            *cache = mir_cache::get_cache(&tcx.crate_name(LOCAL_CRATE).to_string());
        }
        if let Some(cache) = cache.as_mut()
            && let Some(analyzed) = cache.get_cache(&file_hash, &mir_hash)
        {
            log::debug!("MIR cache hit: {fn_id:?}");
            return MirAnalyzerInitResult::Cached(AnalyzeResult {
                file_name,
                file_hash,
                mir_hash,
                analyzed,
            });
        }
        drop(cache);

        let user_vars = mir_transform::collect_user_vars(&source, offset, &facts.body);

        let basic_blocks = mir_transform::collect_basic_blocks(
            fn_id,
            &source,
            offset,
            &facts.body.basic_blocks,
            tcx.sess.source_map(),
        );

        let borrow_data = mir_transform::BorrowMap::new(&facts.borrow_set);

        let analyzer = Box::pin(async move {
            log::debug!("start re-computing borrow check with dump: true");
            let output_datafrog =
                PoloniusOutput::compute(&input, polonius_engine::Algorithm::DatafrogOpt, true);
            log::debug!("borrow check finished");

            let accurate_live =
                mir_polonius::get_accurate_live(&output_datafrog, &location_table, &basic_blocks);

            let must_live = mir_polonius::get_must_live(
                &output_datafrog,
                &location_table,
                &borrow_data,
                &basic_blocks,
            );

            let (shared_live, mutable_live) = mir_polonius::get_borrow_live(
                &output_datafrog,
                &location_table,
                &borrow_data,
                &basic_blocks,
            );

            let drop_range =
                mir_polonius::drop_range(&output_datafrog, &location_table, &basic_blocks);

            Self {
                file_name,
                local_decls,
                user_vars,
                input,
                basic_blocks,
                fn_id,
                file_hash,
                mir_hash,
                accurate_live,
                must_live,
                shared_live,
                mutable_live,
                drop_range,
            }
        });
        MirAnalyzerInitResult::Analyzer(analyzer)
    }

    fn collect_decls(&self) -> Vec<MirDecl> {
        let user_vars = &self.user_vars;
        let lives = &self.accurate_live;
        let must_live_at = &self.must_live;

        let drop_range = &self.drop_range;
        self.local_decls
            .iter()
            .map(|(local, ty)| {
                let ty = ty.clone();
                let must_live_at = must_live_at.get(local).cloned().unwrap_or(Vec::new());
                let lives = lives.get(local).cloned().unwrap_or(Vec::new());
                let shared_borrow = self.shared_live.get(local).cloned().unwrap_or(Vec::new());
                let mutable_borrow = self.mutable_live.get(local).cloned().unwrap_or(Vec::new());
                let drop = self.is_drop(*local);
                let drop_range = drop_range.get(local).cloned().unwrap_or(Vec::new());
                let fn_local = FnLocal::new(local.as_u32(), self.fn_id.local_def_index.as_u32());
                if let Some((span, name)) = user_vars.get(local).cloned() {
                    MirDecl::User {
                        local: fn_local,
                        name,
                        span,
                        ty,
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        must_live_at,
                        drop,
                        drop_range,
                    }
                } else {
                    MirDecl::Other {
                        local: fn_local,
                        ty,
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        drop,
                        drop_range,
                        must_live_at,
                    }
                }
            })
            .collect()
    }

    fn is_drop(&self, local: Local) -> bool {
        for (drop_local, _) in &self.input.var_dropped_at {
            if *drop_local == local {
                return true;
            }
        }
        false
    }

    #[must_use]
    pub fn analyze(self) -> AnalyzeResult {
        let decls = self.collect_decls();
        let basic_blocks = self.basic_blocks;

        AnalyzeResult {
            file_name: self.file_name,
            file_hash: self.file_hash,
            mir_hash: self.mir_hash,
            analyzed: Function {
                fn_id: self.fn_id.local_def_index.as_u32(),
                basic_blocks,
                decls,
            },
        }
    }
}
