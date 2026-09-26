//! What the database tests share: scratch directories, the digests of their
//! bytes, and a look at a database file from outside the store.

use crate::{
    artifact::Digest,
    store::{Database, Error},
};
use rusqlite::Connection;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// SHA-256 of `abc`.
pub(super) const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
/// SHA-256 of `hello`.
pub(super) const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
/// SHA-256 of the empty input.
pub(super) const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: after the databases a test opened in
/// it, which it declares later.
pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-store-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The database file of this directory.
    pub(super) fn database(&self) -> PathBuf {
        self.0.join("kernel.sqlite3")
    }

    /// The artifact root of this directory.
    pub(super) fn artifacts(&self) -> PathBuf {
        self.0.join("artifacts")
    }

    /// The database of this directory, with the binary's migrations.
    pub(super) fn open(&self) -> Database {
        Database::open(&self.database(), &self.artifacts()).unwrap()
    }

    /// The database of this directory, with `migrations` instead.
    pub(super) fn open_with(&self, migrations: &[(&str, &str)]) -> Result<Database, Error> {
        Database::open_with(&self.database(), &self.artifacts(), migrations)
    }

    /// A connection of the test's own to the database file.
    pub(super) fn outside(&self) -> Connection {
        Connection::open(self.database()).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Where the artifact store keeps `digest` under `root`.
pub(super) fn stored(root: &Path, digest: &Digest) -> PathBuf {
    let hex = digest.as_str();
    root.join("sha256")
        .join(&hex[..2])
        .join(&hex[2..4])
        .join(hex)
}

/// The pins the database records for `digest`, if it records the artifact.
pub(super) fn pins(database: &Database, digest: &Digest) -> Option<u64> {
    database
        .artifact(digest)
        .unwrap()
        .map(|artifact| artifact.pins)
}
