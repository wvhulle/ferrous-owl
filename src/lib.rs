#![feature(rustc_private)]

extern crate indexmap;
extern crate polonius_engine;
extern crate rustc_borrowck;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_query_system;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_stable_hash;
extern crate rustc_type_ir;
extern crate smallvec;

mod cli;
mod lsp_decoration;
mod lsp_progress;
mod lsp_server;
mod lsp_workspace;
mod mir_analysis;
mod mir_cache;
mod mir_polonius;
mod mir_transform;
mod models;
mod range_ops;
mod rustc_wrapper;
mod test_framework;
mod text_conversion;
mod toolchain;

pub use cli::Cli;
pub use rustc_wrapper::run_as_rustc_wrapper;
pub use test_framework::{
    DecoKind, ExpectedDeco, LspClient, TestCase, cleanup_workspace, run_test, setup_workspace,
};
