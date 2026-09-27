//! Owns private scratch directories used by synthetic-gate tests.

use std::{
    env, fs,
    io::{self, ErrorKind},
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) struct TestDirectory {
    pub(super) path: PathBuf,
}

impl TestDirectory {
    pub(super) fn new(prefix: &str) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = env::temp_dir().join(format!(
                "{prefix}-{}-{}",
                process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}
