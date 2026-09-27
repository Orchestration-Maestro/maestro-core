//! Atomic derived-input, member and readiness writes.

use super::support::SearchDb;
use crate::{
    generation::NewGeneration,
    retrieval::{Error, ReadControl, SearchInput, SearchMember, SearchProjection, SearchRead},
    scope::{Right, Scope},
    store::{self, Database},
};
use std::{
    slice,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn new_migration_leaves_derived_search_tables_empty() {
    let search = SearchDb::new("Install the tool with --force.");
    let reader = search.database.reader().unwrap();
    for table in [
        "chunk_search_inputs",
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
    let count = reader
        .query_row("SELECT count(*) FROM chunk_search_fts", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn prepared_input_digest_is_checked_before_any_row_is_written() {
    let search = SearchDb::new("Install the tool with --force.");
    let invalid = SearchInput {
        chunk_id: search.chunk_id.clone(),
        prepared_input: "different bytes".to_owned(),
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

#[test]
fn member_write_requires_collection_scope_and_valid_representatives() {
    let search = SearchDb::new("Install the tool with --force.");
    let source_scope: Scope = "workspace/default/collection/ctm/source/docs"
        .parse()
        .unwrap();
    search
        .database
        .grant("source-reader", &source_scope, Right::Read, "test")
        .unwrap();
    let source_scopes = search.database.visible("source-reader").unwrap();
    let member = SearchMember {
        revision_id: "rev-a".to_owned(),
        representative_revision_id: "rev-a".to_owned(),
    };
    assert!(matches!(
        search.database.record_search_members(
            &source_scopes,
            &search.generation.chunk_set_id,
            slice::from_ref(&member),
        ),
        Err(Error::UnknownOrInaccessible)
    ));
    assert!(matches!(
        search
            .database
            .record_search_members(&search.scopes, &search.generation.chunk_set_id, &[],),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        search.database.record_search_members(
            &search.scopes,
            &search.generation.chunk_set_id,
            &[SearchMember {
                representative_revision_id: "missing".to_owned(),
                ..member.clone()
            }],
        ),
        Err(Error::InvalidInput(_))
    ));
    search
        .database
        .record_search_members(
            &search.scopes,
            &search.generation.chunk_set_id,
            slice::from_ref(&member),
        )
        .unwrap();
    search
        .database
        .record_search_members(
            &search.scopes,
            &search.generation.chunk_set_id,
            slice::from_ref(&member),
        )
        .unwrap();
    assert!(matches!(
        search.database.record_search_members(
            &search.scopes,
            &search.generation.chunk_set_id,
            &[member.clone(), member],
        ),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn member_list_rejects_a_self_mapped_revision_without_owned_chunks() {
    let search = SearchDb::new("Install the tool with --force.");
    search.add_duplicate_document();
    let members = [
        SearchMember {
            revision_id: "rev-a".to_owned(),
            representative_revision_id: "rev-a".to_owned(),
        },
        SearchMember {
            revision_id: "rev-b".to_owned(),
            representative_revision_id: "rev-b".to_owned(),
        },
    ];
    assert!(matches!(
        search.database.record_search_members(
            &search.scopes,
            &search.generation.chunk_set_id,
            &members,
        ),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn derived_rows_and_readiness_markers_reject_update_replace_and_delete() {
    let search = SearchDb::new("Install the tool with --force.");
    search.ready();
    for statement in [
        "UPDATE chunk_search_inputs SET prepared_input = 'changed'",
        "DELETE FROM chunk_search_inputs",
        "INSERT OR REPLACE INTO chunk_search_inputs
         (chunk_set_id, chunk_id, prepared_input) VALUES ('set-a', 'chunk-a', 'changed')",
        "INSERT OR REPLACE INTO chunk_search_inputs
         (rowid, chunk_set_id, chunk_id, prepared_input)
         VALUES (1, 'set-a', 'different-chunk', 'changed')",
        "UPDATE chunk_set_members SET representative_revision_id = 'other'",
        "DELETE FROM chunk_set_members",
        "INSERT OR REPLACE INTO chunk_set_members
         (chunk_set_id, revision_id, representative_revision_id)
         VALUES ('set-a', 'rev-a', 'rev-a')",
        "INSERT OR REPLACE INTO chunk_set_members
         (rowid, chunk_set_id, revision_id, representative_revision_id)
         VALUES (1, 'set-a', 'different-revision', 'rev-a')",
        "UPDATE generation_search SET identifier_profile = 'other'",
        "UPDATE generation_search SET ready = 0",
        "DELETE FROM generation_search",
        "INSERT OR REPLACE INTO generation_search (generation_id, identifier_profile)
         VALUES (1, 'other')",
    ] {
        assert!(write_fails(&search.database, statement), "{statement}");
    }
    assert_eq!(input_count(&search.database), 1);
    let reader = search.database.reader().unwrap();
    let matches = reader
        .query_row(
            "SELECT count(*) FROM chunk_search_fts WHERE chunk_search_fts MATCH ?1",
            [r#""--force""#],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(matches, 1);
}

#[test]
fn search_chunks_is_pinned_scoped_versioned_and_independent_of_search_readiness() {
    let search = SearchDb::new("Install the tool with --force.");
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version: None,
        control: &control,
    };
    let ids = vec![search.chunk_id.clone(), search.chunk_id.clone()];
    let chunks = search.database.search_chunks(&read, &ids).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-a"]
    );
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.revision_id.as_str())
            .collect::<Vec<_>>(),
        ["rev-a"]
    );

    let filtered = SearchRead {
        version: Some("9.9"),
        ..read
    };
    assert!(
        search
            .database
            .search_chunks(&filtered, &ids)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn generation_marker_is_building_only_idempotent_and_profile_bound() {
    let search = SearchDb::new("Install the tool with --force.");
    assert!(matches!(
        search
            .database
            .complete_generation_search(&search.scopes, search.generation.id),
        Err(Error::ProjectionMissing)
    ));
    assert!(
        search
            .database
            .begin_generation_search(&search.scopes, search.generation.id, "identifiers/1")
            .unwrap()
    );
    assert!(
        !search
            .database
            .begin_generation_search(&search.scopes, search.generation.id, "identifiers/1")
            .unwrap()
    );
    assert!(matches!(
        search.database.begin_generation_search(
            &search.scopes,
            search.generation.id,
            "identifiers/2"
        ),
        Err(Error::ProfileMismatch { .. })
    ));
    search
        .database
        .complete_generation_search(&search.scopes, search.generation.id)
        .unwrap();
    search
        .database
        .complete_generation_search(&search.scopes, search.generation.id)
        .unwrap();
    assert_eq!(
        search
            .database
            .generation_search(&search.scopes, search.generation.id)
            .unwrap(),
        Some(SearchProjection {
            identifier_profile: "identifiers/1".to_owned(),
            ready: true,
        })
    );

    let verified = search
        .database
        .create_generation(&NewGeneration {
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    search.database.verify_generation(verified.id, 0).unwrap();
    assert!(matches!(
        search
            .database
            .begin_generation_search(&search.scopes, verified.id, "identifiers/1",),
        Err(Error::InvalidInput(_))
    ));
}

fn input_count(database: &Database) -> i64 {
    database
        .reader()
        .unwrap()
        .query_row("SELECT count(*) FROM chunk_search_inputs", [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn write_fails(database: &Database, statement: &str) -> bool {
    database
        .write(|transaction| {
            transaction
                .execute(statement, [])
                .map(|_| ())
                .map_err(store::Error::from)
        })
        .is_err()
}
