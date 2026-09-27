//! Atomic derived-input, member and readiness writes.

use super::support::SearchDb;
use crate::{
    retrieval::{Error, SearchInput},
    store::{self, Database},
};
use std::slice;

#[test]
fn migration_installs_an_empty_exact_identifier_index() {
    let search = SearchDb::new("Install the tool with --force.");
    let reader = search.database.reader().unwrap();
    let index_exists: bool = reader
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema
             WHERE type = 'table' AND name = 'chunk_search_identifiers')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(index_exists);
    let table_sql: String = reader
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table'
             AND name = 'chunk_search_identifiers'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(table_sql.contains("WITHOUT ROWID"));
    let mut statement = reader
        .prepare("PRAGMA table_info(chunk_search_identifiers)")
        .unwrap();
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .into_iter()
        .filter(|(_, position)| *position != 0)
        .collect::<Vec<_>>();
    assert_eq!(
        primary_key,
        [
            ("chunk_set_id".to_owned(), 1),
            ("identifier".to_owned(), 2),
            ("chunk_id".to_owned(), 3)
        ]
    );
    for table in [
        "chunk_search_inputs",
        "chunk_search_identifiers",
        "chunk_set_members",
        "generation_search",
    ] {
        let count = reader
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
}

#[test]
fn recording_search_inputs_stores_an_immutable_exact_identifier_set() {
    let search = SearchDb::new("Install the tool with --force.");
    let input = SearchInput {
        chunk_id: search.chunk_id.clone(),
        prepared_input: search.prepared_input.clone(),
        identifiers: vec![
            "--force".to_owned(),
            "one OR two".to_owned(),
            "--force".to_owned(),
        ],
    };
    search
        .database
        .record_search_inputs(
            &search.scopes,
            &search.generation.chunk_set_id,
            slice::from_ref(&input),
        )
        .unwrap();
    let reader = search.database.reader().unwrap();
    let mut statement = reader
        .prepare(
            "SELECT identifier FROM chunk_search_identifiers
             WHERE chunk_set_id = 'set-a' AND chunk_id = 'chunk-a'
             ORDER BY identifier",
        )
        .unwrap();
    let identifiers = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(identifiers, ["--force", "one OR two"]);

    let changed = SearchInput {
        identifiers: vec!["--force".to_owned()],
        ..input
    };
    assert!(matches!(
        search.database.record_search_inputs(
            &search.scopes,
            &search.generation.chunk_set_id,
            slice::from_ref(&changed),
        ),
        Err(Error::InputConflict)
    ));
}

#[test]
fn identical_prepared_input_retries_backfill_identifiers_for_legacy_rows() {
    let search = SearchDb::new("Install the tool with --force.");
    let legacy = SearchInput {
        chunk_id: search.chunk_id.clone(),
        prepared_input: search.prepared_input.clone(),
        identifiers: Vec::new(),
    };
    search
        .database
        .record_search_inputs(&search.scopes, &search.generation.chunk_set_id, &[legacy])
        .unwrap();
    let indexed = SearchInput {
        chunk_id: search.chunk_id.clone(),
        prepared_input: search.prepared_input.clone(),
        identifiers: vec!["--force".to_owned()],
    };
    search
        .database
        .record_search_inputs(&search.scopes, &search.generation.chunk_set_id, &[indexed])
        .unwrap();
    let reader = search.database.reader().unwrap();
    assert_eq!(
        reader
            .query_row(
                "SELECT count(*) FROM chunk_search_identifiers
                 WHERE chunk_set_id = 'set-a' AND identifier = '--force'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[test]
fn prepared_input_digest_is_checked_before_any_row_is_written() {
    let search = SearchDb::new("Install the tool with --force.");
    let invalid = SearchInput {
        chunk_id: search.chunk_id.clone(),
        prepared_input: "different bytes".to_owned(),
        identifiers: Vec::new(),
    };
    assert!(matches!(
        search.database.record_search_inputs(
            &search.scopes,
            &search.generation.chunk_set_id,
            &[invalid],
        ),
        Err(Error::InvalidInput(_))
    ));
    let count = input_count(&search.database);
    assert_eq!(count, 0);

    search.ready();
    assert_eq!(input_count(&search.database), 1);
}

#[test]
fn search_input_batch_accepts_sixty_four_idempotent_items() {
    let search = SearchDb::new("Install the tool with --force.");
    let inputs = (0..64)
        .map(|_| SearchInput {
            chunk_id: search.chunk_id.clone(),
            prepared_input: search.prepared_input.clone(),
            identifiers: Vec::new(),
        })
        .collect::<Vec<_>>();
    search
        .database
        .record_search_inputs(&search.scopes, &search.generation.chunk_set_id, &inputs)
        .unwrap();
    assert_eq!(input_count(&search.database), 1);
}

#[test]
fn stored_input_retries_distinguish_exact_conflicts_and_bad_digests() {
    let search = SearchDb::new("Install the tool with --force.");
    let input = SearchInput {
        chunk_id: search.chunk_id.clone(),
        prepared_input: search.prepared_input.clone(),
        identifiers: Vec::new(),
    };
    search
        .database
        .record_search_inputs(
            &search.scopes,
            &search.generation.chunk_set_id,
            slice::from_ref(&input),
        )
        .unwrap();
    search
        .database
        .record_search_inputs(
            &search.scopes,
            &search.generation.chunk_set_id,
            slice::from_ref(&input),
        )
        .unwrap();

    let corrupted = SearchDb::new("Install the tool with --force.");
    corrupted
        .database
        .write(|transaction| {
            transaction
                .execute(
                    "INSERT INTO chunk_search_inputs
                     (chunk_set_id, chunk_id, prepared_input)
                     VALUES ('set-a', 'chunk-a', 'wrong stored text')",
                    [],
                )
                .map(|_| ())
                .map_err(store::Error::from)
        })
        .unwrap();
    assert!(matches!(
        corrupted.database.record_search_inputs(
            &corrupted.scopes,
            &corrupted.generation.chunk_set_id,
            &[SearchInput {
                chunk_id: corrupted.chunk_id.clone(),
                prepared_input: corrupted.prepared_input.clone(),
                identifiers: Vec::new(),
            }],
        ),
        Err(Error::InputConflict)
    ));
    assert!(matches!(
        corrupted.database.record_search_inputs(
            &corrupted.scopes,
            &corrupted.generation.chunk_set_id,
            &[SearchInput {
                chunk_id: corrupted.chunk_id.clone(),
                prepared_input: "wrong stored text".to_owned(),
                identifiers: Vec::new(),
            }],
        ),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn search_input_batch_is_limited_to_sixty_four() {
    let search = SearchDb::new("Install the tool with --force.");
    let inputs = (0..65)
        .map(|_| SearchInput {
            chunk_id: search.chunk_id.clone(),
            prepared_input: search.prepared_input.clone(),
            identifiers: Vec::new(),
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        search.database.record_search_inputs(
            &search.scopes,
            &search.generation.chunk_set_id,
            &inputs,
        ),
        Err(Error::TooLarge)
    ));
    assert_eq!(input_count(&search.database), 0);
}

pub(super) fn input_count(database: &Database) -> i64 {
    database
        .reader()
        .unwrap()
        .query_row("SELECT count(*) FROM chunk_search_inputs", [], |row| {
            row.get(0)
        })
        .unwrap()
}
