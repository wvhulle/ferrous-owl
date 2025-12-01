use std::{
    env,
    path::{Path, PathBuf},
};

use tokio::process::Command;

const CACHE_DIR_ENV: &str = "FERROUS_OWL_CACHE_DIR";

pub fn set_cache_path(cmd: &mut Command, target_dir: impl AsRef<Path>) {
    cmd.env(CACHE_DIR_ENV, target_dir.as_ref().join("cache"));
}

pub fn get_cache_path() -> Option<PathBuf> {
    env::var(CACHE_DIR_ENV).map(PathBuf::from).ok()
}
