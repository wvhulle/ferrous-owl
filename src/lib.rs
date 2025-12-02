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

pub mod cli;
pub mod compiler;
mod lsp;
mod models;
pub mod test;
mod toolchain;
mod utils;
pub use lsp::backend::Backend;
