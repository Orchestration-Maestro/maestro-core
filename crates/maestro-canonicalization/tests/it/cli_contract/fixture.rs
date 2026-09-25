//! The scratch directory each command-line test runs the tool in.
use serde_json::Value;
use std::{
    env, fs,
    path::PathBuf,
    process::{self, Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

/// How many scratch directories this process has made, to name the next.
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
/// A scratch directory the tool runs in, removed with everything in it when dropped.
pub(super) struct Fixture(pub(super) PathBuf);

impl Fixture {
    /// A new, empty scratch directory under the plain temporary path.
    pub(super) fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "canonicalization-{}-{}",
            process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// Run the tool on `input.md` with output to `out` and the `extra` arguments.
    pub(super) fn run(&self, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_maestro-canonicalization"))
            .current_dir(&self.0)
            .args(["input.md", "--output", "out"])
            .args(extra)
            .output()
            .unwrap()
    }

    /// The saved document a run reported, and its path.
    pub(super) fn result(&self, output: &Output) -> (PathBuf, Value) {
        let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
        let path = self.0.join(summary["canonical_json"].as_str().unwrap());
        let doc = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        (path, doc)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
