//! Chunks: a revision's chunks recorded at once into a building set, each
//! pinning the artifact of its exact prepared input, read back in the order
//! they were recorded, and only inside the scopes that cover their source.

use super::support::{Scratch, chunk, new_set, pins, prepared, reading};
use crate::{
    artifact::Digest,
    chunk_set::{Chunk, ChunkSetState, Error},
    scope::ScopeSet,
    store,
};

/// The chunks of `rev-a`: two, over prepared inputs of their own.
fn chunks_of_a(database: &store::Database) -> Vec<Chunk> {
    let first = prepared(database, "Installing\n\nThe agent listens on port 7005.");
    let second = prepared(database, "Installing\n\nThe default port is 7006.");
    vec![
        chunk("chunk-a1", "rev-a", &first, 12, [0, 40]),
        chunk("chunk-a2", "rev-a", &second, 11, [40, 72]),
    ]
}

#[test]
fn a_revisions_chunks_are_recorded_at_once_with_their_prepared_inputs_pinned() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let shared = prepared(&database, "Glossary");
    let of_b = vec![chunk("chunk-b1", "rev-b", &shared, 3, [0, 8])];
    let of_a = chunks_of_a(&database);
    database.record_chunks("set-a", "rev-b", &of_b).unwrap();
    database.record_chunks("set-a", "rev-a", &of_a).unwrap();
    // In the order they were recorded: rev-b's first.
    let read = database
        .chunks(&ScopeSet::default_workspace(), "set-a")
        .unwrap();
    assert_eq!(read, [of_b.clone(), of_a.clone()].concat());
    for recorded in &read {
        assert_eq!(pins(&database, &recorded.digest), 1);
    }
    // The same chunks again change nothing, pins included.
    database.record_chunks("set-a", "rev-a", &of_a).unwrap();
    assert_eq!(
        database
            .chunks(&ScopeSet::default_workspace(), "set-a")
            .unwrap(),
        read
    );
    assert_eq!(pins(&database, &of_a[0].digest), 1);
}

#[test]
fn one_prepared_input_is_pinned_once_by_each_chunk_that_names_it() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let shared = prepared(&database, "Glossary");
    database
        .record_chunks(
            "set-a",
            "rev-a",
            &[chunk("chunk-a1", "rev-a", &shared, 3, [0, 8])],
        )
        .unwrap();
    database
        .record_chunks(
            "set-a",
            "rev-b",
            &[chunk("chunk-b1", "rev-b", &shared, 3, [0, 8])],
        )
        .unwrap();
    assert_eq!(pins(&database, &shared), 2);
}

#[test]
fn other_chunks_for_a_revision_already_chunked_are_refused_with_nothing_changed() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let of_a = chunks_of_a(&database);
    database.record_chunks("set-a", "rev-a", &of_a).unwrap();
    let mut recounted = of_a.clone();
    recounted[1].token_count = 13;
    for other in [recounted, vec![of_a[0].clone()]] {
        let Err(Error::ChunksConflict {
            chunk_set,
            revision,
        }) = database.record_chunks("set-a", "rev-a", &other)
        else {
            panic!("{other:?} was not refused");
        };
        assert_eq!((chunk_set.as_str(), revision.as_str()), ("set-a", "rev-a"));
    }
    assert_eq!(
        database
            .chunks(&ScopeSet::default_workspace(), "set-a")
            .unwrap(),
        of_a
    );
    assert_eq!(pins(&database, &of_a[1].digest), 1);
    assert_eq!(
        Error::ChunksConflict {
            chunk_set: "set-a".to_owned(),
            revision: "rev-a".to_owned(),
        }
        .to_string(),
        "the chunk set set-a holds other chunks of the revision rev-a already"
    );
}

#[test]
fn chunks_are_recorded_only_into_a_building_set() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for id in ["set-a", "set-b"] {
        database.begin_chunk_set(&new_set(id, "ctm")).unwrap();
    }
    let manifest = prepared(&database, "{}");
    database.complete_chunk_set("set-a", &manifest).unwrap();
    database.fail_chunk_set("set-b").unwrap();
    let of_a = chunks_of_a(&database);
    for (id, state) in [
        ("set-a", ChunkSetState::Complete),
        ("set-b", ChunkSetState::Failed),
    ] {
        let Err(Error::NotBuilding {
            chunk_set,
            state: refused,
        }) = database.record_chunks(id, "rev-a", &of_a)
        else {
            panic!("{id} took chunks");
        };
        assert_eq!((chunk_set.as_str(), refused), (id, state));
    }
    let Err(Error::UnknownChunkSet(id)) = database.record_chunks("set-c", "rev-a", &of_a) else {
        panic!("an unknown set took chunks");
    };
    assert_eq!(id, "set-c");
    assert_eq!(pins(&database, &of_a[0].digest), 0);
    assert_eq!(
        Error::NotBuilding {
            chunk_set: "set-a".to_owned(),
            state: ChunkSetState::Complete,
        }
        .to_string(),
        "the chunk set set-a is complete: it takes no more chunks"
    );
}

#[test]
fn a_chunk_of_another_revision_or_collection_or_without_its_prepared_input_is_refused_whole() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let of_a = chunks_of_a(&database);
    let mut unstored = of_a.clone();
    unstored[1].digest = Digest::of(b"never stored");
    assert!(matches!(
        database.record_chunks("set-a", "rev-a", &unstored),
        Err(Error::Store(store::Error::UnknownArtifact(_)))
    ));
    let other = chunk("chunk-z1", "rev-z", &of_a[0].digest, 3, [0, 8]);
    assert!(matches!(
        database.record_chunks("set-a", "rev-z", &[other]),
        Err(Error::Store(store::Error::Sqlite(_)))
    ));
    let mut foreign = of_a.clone();
    foreign[1].revision_id = "rev-b".to_owned();
    let Err(Error::ChunksConflict { revision, .. }) =
        database.record_chunks("set-a", "rev-a", &foreign)
    else {
        panic!("a chunk of rev-b was recorded as rev-a's");
    };
    assert_eq!(revision, "rev-a");
    assert_eq!(
        database
            .chunks(&ScopeSet::default_workspace(), "set-a")
            .unwrap(),
        []
    );
    assert_eq!(pins(&database, &of_a[0].digest), 0);
}

#[test]
fn chunks_are_read_only_inside_the_scopes_that_cover_their_source() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let of_a = chunks_of_a(&database);
    database.record_chunks("set-a", "rev-a", &of_a).unwrap();
    assert_eq!(
        database
            .chunks(&reading(&database, "ctm"), "set-a")
            .unwrap(),
        of_a
    );
    assert_eq!(
        database
            .chunks(&reading(&database, "other"), "set-a")
            .unwrap(),
        []
    );
}
