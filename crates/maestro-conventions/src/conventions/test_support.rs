//! Shared scratch-directory helpers for unit tests.

use std::{env, fs, io, path::PathBuf, process};

/// Creates a fresh test directory beneath the operating system's temp directory.
pub(super) fn scratch(name: &str) -> io::Result<PathBuf> {
    let dir = env::temp_dir().join(format!("policy-{name}-{}", process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}
