use std::{
    collections::HashMap,
    env, error, fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    sync::{LazyLock, Mutex, atomic::AtomicBool},
    thread,
};

use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{mir::ConcreteOpaqueTypes, query::queries, ty::TyCtxt, util::Providers};
use rustc_session::config;
use tempfile::NamedTempFile;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
    task::JoinSet,
};

use crate::{
    mir_analysis::{AnalyzeResult, MirAnalyzer, MirAnalyzerInitResult},
    mir_cache,
    models::{Crate, File, Workspace},
};

#[derive(Debug)]
pub enum AnalysisError {
    RustcPanic,
    CompilationFailed(i32),
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RustcPanic => write!(f, "Rustc panicked during analysis"),
            Self::CompilationFailed(code) => write!(f, "Compilation failed with exit code {code}"),
        }
    }
}

impl error::Error for AnalysisError {}

pub struct AnalysisHandle {
    pub results: mpsc::UnboundedReceiver<Workspace>,
    pub thread: thread::JoinHandle<Result<i32, AnalysisError>>,
}

#[must_use]
pub fn run_as_rustc_wrapper() -> i32 {
    run_compiler(&env::args().collect::<Vec<_>>())
}

#[must_use]
pub fn spawn_analysis(file: &Path, sysroot: &Path) -> AnalysisHandle {
    let (sender, receiver) = mpsc::unbounded_channel();

    let output_file = NamedTempFile::new().expect("Failed to create temp file for compiler output");
    let output_path = output_file.path().to_string_lossy().to_string();

    let mut args = vec![
        env!("CARGO_PKG_NAME").to_string(),
        env!("CARGO_PKG_NAME").to_string(),
        format!("--sysroot={}", sysroot.display()),
        "--crate-type=lib".to_string(),
        format!("-o{output_path}"),
    ];
    args.push(file.to_string_lossy().to_string());

    let thread = thread::Builder::new()
        .name("ferrous-owl-compiler".to_string())
        .stack_size(128 * 1024 * 1024)
        .spawn(move || {
            let _output_guard = output_file;
            *RESULT_SENDER.lock().unwrap() = Some(sender);
            let result = catch_unwind(AssertUnwindSafe(|| run_compiler(&args)));
            *RESULT_SENDER.lock().unwrap() = None;

            result.map_or(Err(AnalysisError::RustcPanic), |exit_code| {
                if exit_code == 0 {
                    Ok(exit_code)
                } else {
                    Err(AnalysisError::CompilationFailed(exit_code))
                }
            })
        })
        .expect("Failed to spawn compiler thread");

    AnalysisHandle {
        results: receiver,
        thread,
    }
}

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);
static TASKS: LazyLock<Mutex<JoinSet<AnalyzeResult>>> =
    LazyLock::new(|| Mutex::new(JoinSet::new()));
static RESULT_SENDER: LazyLock<Mutex<Option<mpsc::UnboundedSender<Workspace>>>> =
    LazyLock::new(|| Mutex::new(None));

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    let worker_threads = thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 8))
        .unwrap_or(4);

    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .thread_stack_size(128 * 1024 * 1024)
        .build()
        .unwrap()
});

fn run_compiler(args: &[String]) -> i32 {
    let is_wrapper_mode = args.first() == args.get(1);
    let args: Vec<String> = if is_wrapper_mode {
        args.iter().skip(1).cloned().collect()
    } else {
        return rustc_driver::catch_with_exit_code(|| {
            rustc_driver::run_compiler(args, &mut PassthroughCallback);
        });
    };

    let is_meta_query = args
        .iter()
        .any(|arg| arg == "-vV" || arg == "--version" || arg.starts_with("--print"));

    if is_meta_query {
        return rustc_driver::catch_with_exit_code(|| {
            rustc_driver::run_compiler(&args, &mut PassthroughCallback);
        });
    }

    rustc_driver::catch_with_exit_code(|| {
        rustc_driver::run_compiler(&args, &mut AnalyzerCallback);
    })
}

struct PassthroughCallback;

impl rustc_driver::Callbacks for PassthroughCallback {}

struct AnalyzerCallback;

impl rustc_driver::Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.using_internal_features = &ATOMIC_TRUE;
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.polonius = config::Polonius::Next;
        config.opts.incremental = None;
        config.override_queries = Some(override_queries);
        config.make_codegen_backend = None;
    }

    fn after_expansion(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'_>,
    ) -> rustc_driver::Compilation {
        let result = rustc_driver::catch_fatal_errors(|| tcx.analysis(()));

        #[allow(clippy::await_holding_lock, reason = "lock duration is minimal")]
        RUNTIME.block_on(async move {
            while let Some(Ok(result)) = { TASKS.lock().unwrap().join_next().await } {
                log::info!("one task joined");
                send_result(tcx, result);
            }
            if let Some(cache) = mir_cache::CACHE.lock().unwrap().as_ref() {
                mir_cache::write_cache(&tcx.crate_name(LOCAL_CRATE).to_string(), cache);
            }
        });

        if result.is_ok() {
            rustc_driver::Compilation::Continue
        } else {
            rustc_driver::Compilation::Stop
        }
    }
}

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.mir_borrowck = mir_borrowck;
}

#[allow(clippy::unnecessary_wraps, reason = "required by rustc query system")]
fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_> {
    log::debug!("start borrowck of {def_id:?}");

    let analyzer = MirAnalyzer::init(tcx, def_id);

    {
        let mut tasks = TASKS.lock().unwrap();
        match analyzer {
            MirAnalyzerInitResult::Cached(cached) => send_result(tcx, cached),
            MirAnalyzerInitResult::Analyzer(analyzer) => {
                tasks.spawn_on(async move { analyzer.await.analyze() }, RUNTIME.handle());
            }
        }

        log::debug!("there are {} tasks", tasks.len());
        while let Some(Ok(result)) = tasks.try_join_next() {
            log::debug!("one task joined");
            send_result(tcx, result);
        }
    }

    for def_id in tcx.nested_bodies_within(def_id) {
        let _ = mir_borrowck(tcx, def_id);
    }

    Ok(tcx
        .arena
        .alloc(ConcreteOpaqueTypes(indexmap::IndexMap::default())))
}

fn send_result(tcx: TyCtxt<'_>, analyzed: AnalyzeResult) {
    if let Some(cache) = mir_cache::CACHE.lock().unwrap().as_mut() {
        cache.insert_cache(
            analyzed.file_hash.clone(),
            analyzed.mir_hash.clone(),
            analyzed.analyzed.clone(),
        );
    }

    let krate = Crate(HashMap::from([(
        analyzed.file_name.clone(),
        File {
            items: vec![analyzed.analyzed],
        },
    )]));
    let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let workspace = Workspace(HashMap::from([(crate_name, krate)]));

    if let Some(sender) = RESULT_SENDER.lock().unwrap().as_ref() {
        let _ = sender.send(workspace);
    } else {
        println!("{}", serde_json::to_string(&workspace).unwrap());
    }
}
