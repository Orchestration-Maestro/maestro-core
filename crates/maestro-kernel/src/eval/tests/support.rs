//! What the report tests share: a scratch database holding the collections
//! `ctm` and `synthetic`, each with a chunk set and a generation, and the
//! reports a test records in it.

use crate::{
    document::Collection,
    eval::NewReport,
    generation::NewGeneration,
    store::{self, Database},
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The digest of the manifest each complete chunk set of the tests names.
const MANIFEST: &str = "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";
/// The generation of `ctm` in the database [`Scratch::open`] gives.
pub(super) const CTM: i64 = 1;
/// The generation of `synthetic` in the database [`Scratch::open`] gives.
pub(super) const SYNTHETIC: i64 = 2;

/// A report's JSON, which the kernel keeps as it is given.
pub(super) const JSON: &[u8] = br#"{"schema":"maestro-eval-report/1","suite":"synthetic"}"#;

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: after the database a test opened in
/// it, which it declares later.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-eval-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The kernel database of this directory, with the collections `ctm` and
    /// `synthetic` recorded, each with a chunk set and a generation, [`CTM`]
    /// and [`SYNTHETIC`].
    pub(super) fn open(&self) -> Database {
        let database = Database::open_in(&self.0).unwrap();
        for id in ["ctm", "synthetic"] {
            let collection = Collection {
                id: id.to_owned(),
                title: format!("The {id} collection"),
                visibility: "private".to_owned(),
                profiles: BTreeMap::new(),
            };
            database.record_collection(&collection).unwrap();
            execute(
                &database,
                &format!(
                    "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id,
                       state)
                     VALUES ('{id}-set', '{id}', 'structural-500-700/1', 'native', 'building')"
                ),
            )
            .unwrap();
            execute(
                &database,
                &format!(
                    "UPDATE chunk_sets SET state = 'complete', manifest_digest = '{MANIFEST}'
                     WHERE id = '{id}-set'"
                ),
            )
            .unwrap();
            database
                .create_generation(&NewGeneration {
                    collection_id: id.to_owned(),
                    chunk_set_id: format!("{id}-set"),
                    embedding_profile: "embed:test".to_owned(),
                    sparse_profile: "bm25-en-fr/1".to_owned(),
                })
                .unwrap();
        }
        database
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The report [`JSON`] of the suite `synthetic` over `generation` of
/// `collection`.
pub(super) fn report(collection: &str, generation: i64) -> NewReport<'_> {
    NewReport {
        collection_id: collection,
        generation,
        suite: "synthetic",
        json: JSON,
    }
}

/// Runs `statement` on the writer and commits what it did.
pub(super) fn execute(database: &Database, statement: &str) -> rusqlite::Result<usize> {
    database
        .write(|transaction| Ok::<_, store::Error>(transaction.execute(statement, [])))
        .unwrap()
}
