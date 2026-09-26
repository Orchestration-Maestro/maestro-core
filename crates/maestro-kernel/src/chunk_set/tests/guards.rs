//! The guards of migration `0007_chunk_sets`: whoever writes, and however,
//! a chunk set keeps its three states, its moves and its identity, and a
//! complete set keeps its chunks. Each trigger is shown refusing raw SQL the
//! kernel's functions never send, one test each.

use super::support::{Scratch, new_set};
use crate::store::{self, Database};

/// A manifest's digest, as a complete set records it.
const MANIFEST: &str = "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// Runs `statement` on the writer and commits what it did, or gives the
/// message of the trigger that refused it.
fn run(database: &Database, statement: &str) -> Result<(), String> {
    database
        .write(|transaction| Ok::<_, store::Error>(transaction.execute_batch(statement)))
        .unwrap()
        .map_err(|error| match error {
            rusqlite::Error::SqliteFailure(_, Some(message)) => message,
            other => other.to_string(),
        })
}

/// The database of `scratch` with the chunk set `set-a` of `ctm` building.
fn building(scratch: &Scratch) -> Database {
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    database
}

/// The database of `scratch` with the chunk set `set-a` of `ctm` holding one
/// chunk of `rev-a`, and complete.
fn complete(scratch: &Scratch) -> Database {
    let database = building(scratch);
    run(&database, &chunk("chunk-1", "rev-a")).unwrap();
    run(
        &database,
        &format!(
            "UPDATE chunk_sets SET state = 'complete', manifest_digest = '{MANIFEST}'
             WHERE id = 'set-a'"
        ),
    )
    .unwrap();
    database
}

/// The raw insert of the chunk `id` of `revision` into `set-a`.
fn chunk(id: &str, revision: &str) -> String {
    format!(
        "INSERT INTO chunks (chunk_set_id, id, revision_id, digest, token_count, span_start,
           span_end)
         VALUES ('set-a', '{id}', '{revision}', '{MANIFEST}', 3, 0, 8)"
    )
}

/// The raw insert of the chunk set `set-b` of `ctm` in `state`, with
/// `manifest` if any.
fn insert(state: &str, manifest: &str) -> String {
    format!(
        "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state,
           manifest_digest)
         VALUES ('set-b', 'ctm', 'mapped-structural-chunks/2', 'router/1:test', '{state}',
           {manifest})"
    )
}

#[test]
fn a_chunk_set_is_inserted_building_without_a_manifest_and_in_no_other_state() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let refusal = "a chunk set begins building, without a manifest";
    for (state, manifest) in [
        ("complete", format!("'{MANIFEST}'")),
        ("complete", "NULL".to_owned()),
        ("failed", "NULL".to_owned()),
        ("building", format!("'{MANIFEST}'")),
        ("finished", "NULL".to_owned()),
    ] {
        assert_eq!(
            run(&database, &insert(state, &manifest)),
            Err(refusal.to_owned())
        );
    }
    run(&database, &insert("building", "NULL")).unwrap();
}

#[test]
fn a_chunk_set_is_never_replaced() {
    let scratch = Scratch::new();
    let database = complete(&scratch);
    let replaced = "INSERT OR REPLACE INTO chunk_sets (id, collection_id, chunk_profile,
         counter_contract_id, state)
       VALUES ('set-a', 'ctm', 'mapped-structural-chunks/2', 'router/1:test', 'building')";
    assert_eq!(
        run(&database, replaced),
        Err("a chunk set is never replaced: its id names it for good".to_owned())
    );
}

#[test]
fn a_chunk_set_keeps_its_identity() {
    let scratch = Scratch::new();
    let database = building(&scratch);
    for column in [
        "id = 'set-z'",
        "collection_id = 'other'",
        "chunk_profile = 'mapped-structural-chunks/3'",
        "counter_contract_id = 'router/1:other'",
    ] {
        let statement = format!(
            "UPDATE chunk_sets SET {column}, state = 'complete', manifest_digest = '{MANIFEST}'
             WHERE id = 'set-a'"
        );
        assert_eq!(
            run(&database, &statement),
            Err("a chunk set keeps its id, collection, chunk profile and counter".to_owned()),
            "{column}"
        );
    }
}

#[test]
fn a_chunk_set_moves_only_from_building_to_complete_with_a_manifest_or_to_failed() {
    let scratch = Scratch::new();
    let database = building(&scratch);
    let refusal = "a chunk set moves only from building, to complete with its manifest or to \
                   failed without one";
    let set = |assignments: &str| format!("UPDATE chunk_sets SET {assignments} WHERE id = 'set-a'");
    for assignments in [
        "state = 'complete'".to_owned(),
        format!("state = 'failed', manifest_digest = '{MANIFEST}'"),
        format!("manifest_digest = '{MANIFEST}'"),
        "state = 'finished'".to_owned(),
        "state = 'building'".to_owned(),
    ] {
        assert_eq!(
            run(&database, &set(&assignments)),
            Err(refusal.to_owned()),
            "{assignments}"
        );
    }
    run(
        &database,
        &set(&format!(
            "state = 'complete', manifest_digest = '{MANIFEST}'"
        )),
    )
    .unwrap();
    // Complete, then failed: each final.
    for assignments in [
        "state = 'building', manifest_digest = NULL".to_owned(),
        "state = 'failed', manifest_digest = NULL".to_owned(),
        format!("manifest_digest = '{}'", MANIFEST.replace('4', "5")),
    ] {
        assert_eq!(
            run(&database, &set(&assignments)),
            Err(refusal.to_owned()),
            "{assignments}"
        );
    }
    run(&database, &insert("building", "NULL")).unwrap();
    let fail = "UPDATE chunk_sets SET state = 'failed' WHERE id = 'set-b'";
    run(&database, fail).unwrap();
    assert_eq!(run(&database, fail), Err(refusal.to_owned()));
}

#[test]
fn a_chunk_set_is_never_deleted() {
    let scratch = Scratch::new();
    let database = building(&scratch);
    assert_eq!(
        run(&database, "DELETE FROM chunk_sets WHERE id = 'set-a'"),
        Err("a chunk set is never deleted: a generation may name it".to_owned())
    );
}

#[test]
fn a_chunk_enters_only_a_building_set_for_a_revision_of_its_collection() {
    let scratch = Scratch::new();
    let database = building(&scratch);
    let refusal = "a chunk enters only a building chunk set, for a revision of the set's \
                   collection";
    // `rev-z` is a revision of the collection `other`.
    assert_eq!(
        run(&database, &chunk("chunk-z", "rev-z")),
        Err(refusal.to_owned())
    );
    run(&database, &chunk("chunk-1", "rev-a")).unwrap();
    run(
        &database,
        &format!(
            "UPDATE chunk_sets SET state = 'complete', manifest_digest = '{MANIFEST}'
             WHERE id = 'set-a'"
        ),
    )
    .unwrap();
    assert_eq!(
        run(&database, &chunk("chunk-2", "rev-a")),
        Err(refusal.to_owned())
    );
}

#[test]
fn a_chunk_is_never_replaced() {
    let scratch = Scratch::new();
    let database = building(&scratch);
    run(&database, &chunk("chunk-1", "rev-a")).unwrap();
    assert_eq!(
        run(
            &database,
            &chunk("chunk-1", "rev-a").replace("INSERT", "INSERT OR REPLACE")
        ),
        Err("a chunk is never replaced: its set holds it as it was counted".to_owned())
    );
}

#[test]
fn a_chunk_never_changes() {
    let scratch = Scratch::new();
    let database = complete(&scratch);
    assert_eq!(
        run(
            &database,
            "UPDATE chunks SET token_count = 4 WHERE id = 'chunk-1'"
        ),
        Err("a chunk never changes: its set holds it as it was counted".to_owned())
    );
}

#[test]
fn a_chunk_is_never_deleted() {
    let scratch = Scratch::new();
    let database = complete(&scratch);
    assert_eq!(
        run(&database, "DELETE FROM chunks WHERE id = 'chunk-1'"),
        Err("a chunk is never deleted: its set holds it as it was counted".to_owned())
    );
}
