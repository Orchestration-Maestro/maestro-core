//! What the generation tests share: a scratch database holding two
//! collections, each with a chunk set, and generations of them moved to the
//! state a test starts from.

use crate::{
    document::Collection,
    generation::{GenerationState, NewGeneration},
    scope::ScopeSet,
    store::{self, Database},
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: after the database a test opened in
/// it, which it declares later.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-generation-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The kernel database of this directory, with the collections `ctm` and
    /// `synthetic` recorded, each with its chunk set `<collection>-set`.
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
                "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id,
                   state)
                 VALUES (?1 || '-set', ?1, 'structural-500-700/1', 'native', 'complete')",
                id,
            )
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

/// A generation of `collection` from its chunk set, embedded by `embed:test`
/// and analysed by `bm25-en-fr/1`.
pub(super) fn new_generation(collection: &str) -> NewGeneration {
    NewGeneration {
        collection_id: collection.to_owned(),
        chunk_set_id: format!("{collection}-set"),
        embedding_profile: "embed:test".to_owned(),
        sparse_profile: "bm25-en-fr/1".to_owned(),
    }
}

/// The id of a new generation of `collection`, moved to `state` by the
/// legal moves: publishing it retires the one published before it, and a
/// failed one fails while it is building.
pub(super) fn generation_in(database: &Database, collection: &str, state: GenerationState) -> i64 {
    let id = database
        .create_generation(&new_generation(collection))
        .unwrap()
        .id;
    if state == GenerationState::Failed {
        database.fail_generation(id).unwrap();
        return id;
    }
    if state != GenerationState::Building {
        database.verify_generation(id, 3).unwrap();
    }
    if matches!(state, GenerationState::Published | GenerationState::Retired) {
        database.publish_generation(id).unwrap();
    }
    if state == GenerationState::Retired {
        database.retire_generation(id).unwrap();
    }
    id
}

/// The state the generation `id` is in.
pub(super) fn state(database: &Database, id: i64) -> GenerationState {
    database
        .generation(&ScopeSet::default_workspace(), id)
        .unwrap()
        .unwrap()
        .state
}

/// Runs `statement` with the parameter `value` on the writer and commits
/// what it did.
pub(super) fn execute(
    database: &Database,
    statement: &str,
    value: &str,
) -> rusqlite::Result<usize> {
    database
        .write(|transaction| Ok::<_, store::Error>(transaction.execute(statement, [value])))
        .unwrap()
}
