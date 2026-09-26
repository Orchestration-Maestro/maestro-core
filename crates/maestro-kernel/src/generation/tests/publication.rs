//! Publication: at most one generation of a collection is published, which
//! the database itself holds, publishing one retires the one published before
//! it in the same transaction, and a failed one is never published.

use super::support::{Scratch, execute, generation_in, state};
use crate::generation::{
    Error,
    GenerationState::{Building, Failed, Published, Retired, Verified},
};
use rusqlite::{Connection, ffi};

/// The time SQLite reads from the clock, in the form the kernel records.
fn now(clock: &Connection) -> String {
    clock
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .unwrap()
}

/// The extended result code of the SQLite failure in `result`.
fn failure(result: rusqlite::Result<usize>) -> i32 {
    match result {
        Err(rusqlite::Error::SqliteFailure(error, _)) => error.extended_code,
        other => panic!("not a SQLite failure: {other:?}"),
    }
}

#[test]
fn a_second_publish_in_one_collection_retires_the_first() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let first = generation_in(&database, "ctm", Verified);
    assert_eq!(database.publish_generation(first).unwrap(), None);
    let second = generation_in(&database, "ctm", Verified);
    // A build to resume and one waiting to be published: neither is retired.
    let building = generation_in(&database, "ctm", Building);
    let verified = generation_in(&database, "ctm", Verified);
    assert_eq!(database.publish_generation(second).unwrap(), Some(first));
    assert_eq!(state(&database, first), Retired);
    assert_eq!(state(&database, second), Published);
    assert_eq!(state(&database, building), Building);
    assert_eq!(state(&database, verified), Verified);
    let published = database.published_generation("ctm").unwrap().unwrap();
    assert_eq!(published.id, second);
    let retired = database.generation(first).unwrap().unwrap();
    assert!(retired.published_at.is_some(), "{retired:?}");
}

#[test]
fn a_failed_generation_neither_holds_back_the_next_publish_nor_resumes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let failed_building = generation_in(&database, "ctm", Failed);
    let failed_verified = generation_in(&database, "ctm", Verified);
    database.fail_generation(failed_verified).unwrap();
    let next = generation_in(&database, "ctm", Verified);
    assert_eq!(
        database.publish_generation(next).unwrap(),
        None,
        "a failed generation was never published, so none is retired"
    );
    assert_eq!(state(&database, next), Published);
    for failed in [failed_building, failed_verified] {
        let kept = database.generation(failed).unwrap().unwrap();
        assert_eq!((kept.state, kept.published_at), (Failed, None));
        // Resuming it would verify it, then publish it.
        let resumed = [
            database.verify_generation(failed, 3),
            database.publish_generation(failed).map(drop),
        ];
        for refused in resumed {
            assert!(
                matches!(refused, Err(Error::IllegalMove { from: Failed, .. })),
                "{refused:?}"
            );
        }
    }
}

#[test]
fn a_publish_in_one_collection_leaves_the_others_published() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let synthetic = generation_in(&database, "synthetic", Published);
    let ctm = generation_in(&database, "ctm", Verified);
    assert_eq!(database.publish_generation(ctm).unwrap(), None);
    assert_eq!(state(&database, synthetic), Published);
    let published = database.published_generation("synthetic").unwrap();
    assert_eq!(published.map(|generation| generation.id), Some(synthetic));
}

#[test]
fn publishing_records_when_the_generation_was_published() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let clock = Connection::open_in_memory().unwrap();
    let id = generation_in(&database, "ctm", Verified);
    let before = now(&clock);
    database.publish_generation(id).unwrap();
    let after = now(&clock);
    let published_at = database
        .generation(id)
        .unwrap()
        .unwrap()
        .published_at
        .unwrap();
    assert!(
        before <= published_at && published_at <= after,
        "{before} <= {published_at} <= {after}"
    );
}

#[test]
fn retiring_the_published_generation_leaves_its_collection_without_one() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let id = generation_in(&database, "ctm", Published);
    assert_eq!(
        database
            .published_generation("ctm")
            .unwrap()
            .map(|generation| generation.id),
        Some(id)
    );
    database.retire_generation(id).unwrap();
    assert_eq!(state(&database, id), Retired);
    assert_eq!(database.published_generation("ctm").unwrap(), None);
}

#[test]
fn the_database_refuses_two_published_generations_of_one_collection_even_written_directly() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let insert = "INSERT INTO generations (collection_id, chunk_set_id, embedding_profile,
                    sparse_profile, state)
                  VALUES (?1, ?1 || '-set', 'embed:test', 'bm25-en-fr/1', 'published')";
    assert_eq!(execute(&database, insert, "ctm"), Ok(1));
    assert_eq!(
        failure(execute(&database, insert, "ctm")),
        ffi::SQLITE_CONSTRAINT_UNIQUE
    );
    assert_eq!(execute(&database, insert, "synthetic"), Ok(1));
    let verified = generation_in(&database, "ctm", Verified);
    assert_eq!(
        failure(execute(
            &database,
            "UPDATE generations SET state = 'published' WHERE id = ?1",
            &verified.to_string(),
        )),
        ffi::SQLITE_CONSTRAINT_UNIQUE
    );
    assert_eq!(state(&database, verified), Verified);
}
