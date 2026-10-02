use crate::files::{self, FileInput, FilePlan};
use crate::policy::workspace::{CheckedTrust, TrustBoundaries, WorkspaceTrust};
use maestro_test_scratch::scratch_directory;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(super) struct Scratch {
    pub(super) path: PathBuf,
}

impl Scratch {
    pub(super) fn new() -> Self {
        let path = scratch_directory().unwrap();
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

/// Synthetic approved roots for legacy writer contracts; never a production authority adapter.
pub(crate) fn with_trust<T>(root: &Path, run: impl FnOnce(&CheckedTrust<'_>) -> T) -> T {
    struct Root(PathBuf);
    impl WorkspaceTrust for Root {
        fn containing_root(&self, path: &Path) -> Option<PathBuf> {
            path.starts_with(&self.0).then(|| self.0.clone())
        }
    }
    let canonical = root.canonicalize().unwrap();
    let boundaries = TrustBoundaries::new(canonical.parent().unwrap(), &[]).unwrap();
    let adapter = Root(canonical);
    run(&CheckedTrust::new(&adapter, &boundaries))
}

/// Preview with a synthetic approved-root adapter while preserving production path policy.
pub(crate) fn preview(
    root: &Path,
    files: impl IntoIterator<Item = FileInput>,
) -> io::Result<FilePlan> {
    with_trust(root, |trust| FilePlan::preview(root, files, trust))
}

/// Apply with synthetic authority for pre-existing filesystem contract tests.
pub(crate) fn apply(root: &Path, plan: &FilePlan) -> io::Result<()> {
    with_trust(root, |trust| files::apply(root, plan, trust))
}

/// Inject durable-stage interruptions without skipping the mandatory policy floor.
pub(crate) fn apply_with_failure(
    root: &Path,
    plan: &FilePlan,
    fail: Option<usize>,
) -> io::Result<()> {
    with_trust(root, |trust| {
        files::apply_with_failure(root, plan, fail, trust)
    })
}

/// Recover with the same synthetic approved-root authority as the original create.
pub(crate) fn recover(root: &Path, id: &str) -> io::Result<()> {
    with_trust(root, |trust| files::recover(root, id, trust))
}

/// Remove with synthetic authority, still requiring the genuine ownership checks.
pub(crate) fn remove(root: &Path, id: &str) -> io::Result<()> {
    with_trust(root, |trust| files::remove(root, id, trust))
}
