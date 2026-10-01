//! A chunk set's lifecycle: begun building, found again just so by a rerun,
//! then complete with its manifest pinned, or failed; neither ever moves
//! again, and an id is never taken for another collection, profile or
//! counter.

use super::support::{Scratch, new_set, pins, prepared, reading};
use crate::{
    artifact::Digest,
    chunk_set::{
        ChunkSet,
        ChunkSetState::{self, Building, Complete, Failed},
        Error, NewChunkSet,
    },
    scope::ScopeSet,
    store,
};
use std::error;

/// The chunk set `new` begins, as the kernel records it.
fn building(new: &NewChunkSet<'_>) -> ChunkSet {
    ChunkSet {
        id: new.id.to_owned(),
        collection_id: new.collection_id.to_owned(),
        chunk_profile: new.chunk_profile.to_owned(),
        counter_contract_id: new.counter_contract_id.to_owned(),
        state: Building,
        manifest_digest: None,
    }
}

#[test]
fn a_chunk_set_begins_building_and_a_rerun_finds_it_just_so() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let new = new_set("set-a", "ctm");
    assert_eq!(database.begin_chunk_set(&new).unwrap(), building(&new));
    assert_eq!(database.begin_chunk_set(&new).unwrap(), building(&new));
    let everything = ScopeSet::default_workspace();
    assert_eq!(
        database.chunk_set(&everything, "set-a").unwrap(),
        Some(building(&new))
    );
    assert_eq!(database.chunk_set(&everything, "set-b").unwrap(), None);
}

#[test]
fn an_id_taken_by_another_collection_profile_or_counter_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let new = new_set("set-a", "ctm");
    database.begin_chunk_set(&new).unwrap();
    for other in [
        NewChunkSet {
            collection_id: "other",
            ..new
        },
        NewChunkSet {
            chunk_profile: "mapped-structural-chunks/3",
            ..new
        },
        NewChunkSet {
            counter_contract_id: "router/1:another",
            ..new
        },
    ] {
        let Err(Error::Conflict(id)) = database.begin_chunk_set(&other) else {
            panic!("{other:?} was not refused");
        };
        assert_eq!(id, "set-a");
    }
    assert_eq!(
        database
            .chunk_set(&ScopeSet::default_workspace(), "set-a")
            .unwrap(),
        Some(building(&new))
    );
    assert_eq!(
        Error::Conflict("set-a".to_owned()).to_string(),
        "the chunk set set-a is recorded already for another collection, chunk profile or \
         counter"
    );
}

#[test]
fn a_chunk_set_of_an_unrecorded_collection_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let refused = database.begin_chunk_set(&new_set("set-a", "missing"));
    assert!(matches!(
        refused,
        Err(Error::Store(store::Error::Sqlite(_)))
    ));
    assert_eq!(
        database
            .chunk_set(&ScopeSet::default_workspace(), "set-a")
            .unwrap(),
        None
    );
}

#[test]
fn a_building_set_completes_with_its_manifest_pinned_and_never_moves_again() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let new = new_set("set-a", "ctm");
    database.begin_chunk_set(&new).unwrap();
    let manifest = prepared(&database, "{\"schema\":\"maestro-chunk-set/1\"}");
    database.complete_chunk_set("set-a", &manifest).unwrap();
    let complete = ChunkSet {
        state: Complete,
        manifest_digest: Some(manifest.clone()),
        ..building(&new)
    };
    let everything = ScopeSet::default_workspace();
    assert_eq!(
        database.chunk_set(&everything, "set-a").unwrap(),
        Some(complete.clone())
    );
    assert_eq!(pins(&database, &manifest), 1);
    // A rerun finds it complete, and nothing moves it again.
    assert_eq!(database.begin_chunk_set(&new).unwrap(), complete);
    assert_illegal(
        database.complete_chunk_set("set-a", &manifest),
        Complete,
        Complete,
    );
    assert_illegal(database.fail_chunk_set("set-a"), Complete, Failed);
    assert_eq!(
        database.chunk_set(&everything, "set-a").unwrap(),
        Some(complete)
    );
    assert_eq!(pins(&database, &manifest), 1);
}

#[test]
fn a_building_set_fails_and_never_moves_again() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let new = new_set("set-a", "ctm");
    database.begin_chunk_set(&new).unwrap();
    database.fail_chunk_set("set-a").unwrap();
    let failed = ChunkSet {
        state: Failed,
        ..building(&new)
    };
    assert_eq!(database.begin_chunk_set(&new).unwrap(), failed);
    let manifest = prepared(&database, "{}");
    assert_illegal(
        database.complete_chunk_set("set-a", &manifest),
        Failed,
        Complete,
    );
    assert_illegal(database.fail_chunk_set("set-a"), Failed, Failed);
    assert_eq!(pins(&database, &manifest), 0);
    assert_eq!(
        database
            .chunk_set(&ScopeSet::default_workspace(), "set-a")
            .unwrap(),
        Some(failed)
    );
}

#[test]
fn an_unknown_set_is_neither_completed_nor_failed() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let manifest = prepared(&database, "{}");
    for refused in [
        database.complete_chunk_set("set-a", &manifest),
        database.fail_chunk_set("set-a"),
    ] {
        let Err(Error::UnknownChunkSet(id)) = refused else {
            panic!("{refused:?}");
        };
        assert_eq!(id, "set-a");
    }
    assert_eq!(pins(&database, &manifest), 0);
    assert_eq!(
        Error::UnknownChunkSet("set-a".to_owned()).to_string(),
        "no chunk set set-a is recorded"
    );
}

#[test]
fn a_manifest_that_is_not_stored_leaves_the_set_building() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let new = new_set("set-a", "ctm");
    database.begin_chunk_set(&new).unwrap();
    let missing = Digest::of(b"never stored");
    let refused = database.complete_chunk_set("set-a", &missing);
    assert!(matches!(
        refused,
        Err(Error::Store(store::Error::UnknownArtifact(_)))
    ));
    assert_eq!(
        database
            .chunk_set(&ScopeSet::default_workspace(), "set-a")
            .unwrap(),
        Some(building(&new))
    );
}

#[test]
fn a_chunk_set_is_read_only_inside_the_scopes_that_cover_its_collection() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let new = new_set("set-a", "ctm");
    database.begin_chunk_set(&new).unwrap();
    assert_eq!(
        database
            .chunk_set(&reading(&database, "ctm"), "set-a")
            .unwrap(),
        Some(building(&new))
    );
    assert_eq!(
        database
            .chunk_set(&reading(&database, "other"), "set-a")
            .unwrap(),
        None
    );
}

#[test]
fn each_state_is_named_as_its_column_holds_it() {
    let names: Vec<String> = [Building, Complete, Failed]
        .iter()
        .map(ChunkSetState::to_string)
        .collect();
    assert_eq!(names, ["building", "complete", "failed"]);
    assert_eq!(
        Error::IllegalMove {
            chunk_set: "set-a".to_owned(),
            from: Complete,
            to: Failed,
        }
        .to_string(),
        "the chunk set set-a cannot move from complete to failed: a chunk set moves only from \
         building to complete or to failed"
    );
}

/// Asserts that `result` refused to move the chunk set `set-a` from `from`
/// to `to`.
fn assert_illegal(result: Result<(), Error>, from: ChunkSetState, to: ChunkSetState) {
    let Err(Error::IllegalMove {
        chunk_set,
        from: refused_from,
        to: refused_to,
    }) = result
    else {
        panic!("{result:?} is no illegal move");
    };
    assert_eq!(
        (chunk_set.as_str(), refused_from, refused_to),
        ("set-a", from, to)
    );
}

#[test]
fn a_store_failure_keeps_its_cause_and_no_other_refusal_has_one() {
    for refusal in [
        Error::UnknownChunkSet("set-a".to_owned()),
        Error::Conflict("set-a".to_owned()),
        Error::IllegalMove {
            chunk_set: "set-a".to_owned(),
            from: Failed,
            to: Complete,
        },
        Error::NotBuilding {
            chunk_set: "set-a".to_owned(),
            state: Failed,
        },
        Error::ChunksConflict {
            chunk_set: "set-a".to_owned(),
            revision: "rev-a".to_owned(),
        },
    ] {
        assert!(error::Error::source(&refusal).is_none(), "{refusal}");
    }
    let inner = || store::Error::Sqlite(rusqlite::Error::InvalidQuery);
    let wrapped = Error::from(inner());
    assert_eq!(wrapped.to_string(), inner().to_string());
    let reason = error::Error::source(&wrapped).map(ToString::to_string);
    assert_eq!(reason, Some(rusqlite::Error::InvalidQuery.to_string()));
    assert!(
        matches!(
            Error::from(rusqlite::Error::InvalidQuery),
            Error::Store(store::Error::Sqlite(rusqlite::Error::InvalidQuery))
        ),
        "a SQLite error is the store's"
    );
}
