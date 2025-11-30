//! Compiler integration module for MIR analysis
//!
//! This module contains the rustc integration code that was previously in a separate binary.
//! It uses `#![feature(rustc_private)]` to access rustc internals for ownership/lifetime analysis.

mod analyze;
mod cache;

pub use analyze::{AnalyzeResult, MirAnalyzer, MirAnalyzerInitResult};

use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{mir::ConcreteOpaqueTypes, query::queries, ty::TyCtxt, util::Providers};
use rustc_session::config;

use crate::models::*;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{LazyLock, Mutex, atomic::AtomicBool};
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
    task::JoinSet,
};

pub struct RustcCallback;
impl rustc_driver::Callbacks for RustcCallback {}

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);
static TASKS: LazyLock<Mutex<JoinSet<AnalyzeResult>>> =
    LazyLock::new(|| Mutex::new(JoinSet::new()));

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    let worker_threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 8))
        .unwrap_or(4);

    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .thread_stack_size(128 * 1024 * 1024)
        .build()
        .unwrap()
});

/// Channel for sending analysis results back to the caller
static RESULT_SENDER: LazyLock<Mutex<Option<mpsc::UnboundedSender<Workspace>>>> =
    LazyLock::new(|| Mutex::new(None));

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.mir_borrowck = mir_borrowck;
}

fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_> {
    log::info!("start borrowck of {def_id:?}");

    let analyzer = MirAnalyzer::init(tcx, def_id);

    {
        let mut tasks = TASKS.lock().unwrap();
        match analyzer {
            MirAnalyzerInitResult::Cached(cached) => {
                handle_analyzed_result(tcx, cached);
            }
            MirAnalyzerInitResult::Analyzer(analyzer) => {
                tasks.spawn_on(async move { analyzer.await.analyze() }, RUNTIME.handle());
            }
        }

        log::info!("there are {} tasks", tasks.len());
        while let Some(Ok(result)) = tasks.try_join_next() {
            log::info!("one task joined");
            handle_analyzed_result(tcx, result);
        }
    }

    for def_id in tcx.nested_bodies_within(def_id) {
        let _ = mir_borrowck(tcx, def_id);
    }

    Ok(tcx
        .arena
        .alloc(ConcreteOpaqueTypes(indexmap::IndexMap::default())))
}

pub struct AnalyzerCallback;
impl rustc_driver::Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.using_internal_features = &ATOMIC_TRUE;
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.polonius = config::Polonius::Next;
        config.opts.incremental = None;
        config.override_queries = Some(override_queries);
        config.make_codegen_backend = None;
    }
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        let result = rustc_driver::catch_fatal_errors(|| tcx.analysis(()));

        #[allow(clippy::await_holding_lock)]
        RUNTIME.block_on(async move {
            while let Some(Ok(result)) = { TASKS.lock().unwrap().join_next().await } {
                log::info!("one task joined");
                handle_analyzed_result(tcx, result);
            }
            if let Some(cache) = cache::CACHE.lock().unwrap().as_ref() {
                cache::write_cache(&tcx.crate_name(LOCAL_CRATE).to_string(), cache);
            }
        });

        if result.is_ok() {
            rustc_driver::Compilation::Continue
        } else {
            rustc_driver::Compilation::Stop
        }
    }
}

pub fn handle_analyzed_result(tcx: TyCtxt<'_>, analyzed: AnalyzeResult) {
    if let Some(cache) = cache::CACHE.lock().unwrap().as_mut() {
        cache.insert_cache(
            analyzed.file_hash.clone(),
            analyzed.mir_hash.clone(),
            analyzed.analyzed.clone(),
        );
    }
    let krate = Crate(HashMap::from([(
        analyzed.file_name.to_owned(),
        File {
            items: vec![analyzed.analyzed],
        },
    )]));
    let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let ws = Workspace(HashMap::from([(crate_name.clone(), krate)]));

    // Send result through channel if available, otherwise print to stdout
    if let Some(sender) = RESULT_SENDER.lock().unwrap().as_ref() {
        let _ = sender.send(ws);
    } else {
        println!("{}", serde_json::to_string(&ws).unwrap());
    }
}

/// Run the compiler with analysis callbacks
/// Returns exit code
pub fn run_compiler_with_args(args: &[String]) -> i32 {
    let mut args = args.to_vec();

    // by using `RUSTC_WORKSPACE_WRAPPER`, arguments will be as follows:
    // For dependencies: rustowlc [args...]
    // For user workspace: rustowlc rustowlc [args...]
    // So we skip analysis if currently-compiling crate is one of the dependencies
    if args.first() == args.get(1) {
        args = args.into_iter().skip(1).collect();
    } else {
        return rustc_driver::catch_with_exit_code(|| {
            rustc_driver::run_compiler(&args, &mut RustcCallback)
        });
    }

    for arg in &args {
        // utilize default rustc to avoid unexpected behavior if these arguments are passed
        if arg == "-vV" || arg == "--version" || arg.starts_with("--print") {
            return rustc_driver::catch_with_exit_code(|| {
                rustc_driver::run_compiler(&args, &mut RustcCallback)
            });
        }
    }

    rustc_driver::catch_with_exit_code(|| {
        rustc_driver::run_compiler(&args, &mut AnalyzerCallback);
    })
}

/// Error type for analysis failures
#[derive(Debug)]
pub enum AnalysisError {
    ThreadPanic,
    RustcPanic,
    CompilationFailed(i32),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::ThreadPanic => write!(f, "Analysis thread panicked"),
            AnalysisError::RustcPanic => write!(f, "Rustc panicked during analysis"),
            AnalysisError::CompilationFailed(code) => {
                write!(f, "Compilation failed with exit code {code}")
            }
        }
    }
}

impl std::error::Error for AnalysisError {}

/// Run compiler analysis in a separate thread with panic handling
/// Returns a receiver for workspace analysis results
pub fn run_compiler_in_thread(
    args: Vec<String>,
) -> (
    mpsc::UnboundedReceiver<Workspace>,
    std::thread::JoinHandle<Result<i32, AnalysisError>>,
) {
    let (sender, receiver) = mpsc::unbounded_channel();

    let handle = std::thread::Builder::new()
        .name("rustowl-compiler".to_string())
        .stack_size(128 * 1024 * 1024)
        .spawn(move || {
            // Set up the result sender
            *RESULT_SENDER.lock().unwrap() = Some(sender);

            let result = catch_unwind(AssertUnwindSafe(|| run_compiler_with_args(&args)));

            // Clear the sender
            *RESULT_SENDER.lock().unwrap() = None;

            match result {
                Ok(exit_code) => {
                    if exit_code == 0 {
                        Ok(exit_code)
                    } else {
                        Err(AnalysisError::CompilationFailed(exit_code))
                    }
                }
                Err(_) => Err(AnalysisError::RustcPanic),
            }
        })
        .expect("Failed to spawn compiler thread");

    (receiver, handle)
}
