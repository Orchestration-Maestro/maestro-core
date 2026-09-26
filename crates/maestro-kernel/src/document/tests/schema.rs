//! The documents migration: the ten pipeline tables of 01 §11, all strict,
//! each refusing a row whose parent is missing and a value outside its
//! contract, whichever task writes it.

use super::support::{Scratch, failure, revision};
use crate::{
    document::RevisionStatus,
    store::{Database, Error},
};
use rusqlite::{Connection, ffi};

/// The migration, as the binary embeds it.
const MIGRATION: &str = include_str!("../../../migrations/0004_documents.sql");

/// Every table the migration creates, in name order: those of 01 §11 but the
/// jobs, the events, and S6's frontier items and captures.
const TABLES: [&str; 10] = [
    "chunk_sets",
    "chunks",
    "collections",
    "documents",
    "generations",
    "near_dup_groups",
    "occurrences",
    "quality_dispositions",
    "revisions",
    "sources",
];

/// Rows whose parent is missing, one for each foreign key; the row's other
/// parents exist.
const ORPHANS: [&str; 12] = [
    "INSERT INTO sources (collection_id, id, kind, reference, profiles_json)
     VALUES ('missing', 'docs', 'import', 'manifest', '{}')",
    "INSERT INTO documents (id, collection_id, source_id, source_ref)
     VALUES ('doc-b', 'ctm', 'missing', 'https://example.org/b')",
    "INSERT INTO revisions (id, document_id, original_digest, canonical_digest, status,
       metadata_json)
     VALUES ('rev-b', 'missing', 'original', 'canonical', 'valid', '{}')",
    "INSERT INTO quality_dispositions (revision_id, disposition, reasons_json, rule_ids,
       decided_by)
     VALUES ('missing', 'accepted', '[]', '[]', 'the gate')",
    "INSERT INTO occurrences (revision_id, collection_id, source_id, source_ref)
     VALUES ('missing', 'ctm', 'docs', 'https://example.org/a')",
    "INSERT INTO occurrences (revision_id, collection_id, source_id, source_ref)
     VALUES ('rev-a', 'ctm', 'missing', 'https://example.org/a')",
    "INSERT INTO near_dup_groups (group_id, revision_id, jaccard)
     VALUES ('group', 'missing', 0.9)",
    "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
     VALUES ('set-b', 'missing', 'profile', 'counter', 'complete')",
    "INSERT INTO chunks (chunk_set_id, id, revision_id, digest, token_count, span_start,
       span_end)
     VALUES ('missing', 'chunk', 'rev-a', 'digest', 1, 0, 1)",
    "INSERT INTO chunks (chunk_set_id, id, revision_id, digest, token_count, span_start,
       span_end)
     VALUES ('set-a', 'chunk', 'missing', 'digest', 1, 0, 1)",
    "INSERT INTO generations (collection_id, chunk_set_id, embedding_profile, sparse_profile)
     VALUES ('missing', 'set-a', 'embed', 'bm25-en-fr/1')",
    "INSERT INTO generations (collection_id, chunk_set_id, embedding_profile, sparse_profile)
     VALUES ('ctm', 'missing', 'embed', 'bm25-en-fr/1')",
];

/// A row of each table the later tasks write, every parent present.
const ROWS: [&str; 5] = [
    "INSERT INTO quality_dispositions (revision_id, disposition, reasons_json, rule_ids,
       decided_by)
     VALUES ('rev-a', 'accepted_with_warnings', '[\"short\"]', '[\"Q-001\"]', 'the gate')",
    "INSERT INTO occurrences (revision_id, collection_id, source_id, source_ref)
     VALUES ('rev-a', 'ctm', 'docs', 'https://example.org/copy-of-a')",
    "INSERT INTO near_dup_groups (group_id, revision_id, jaccard)
     VALUES ('group', 'rev-a', 0.9)",
    "INSERT INTO chunks (chunk_set_id, id, revision_id, section_id, digest, token_count,
       span_start, span_end)
     VALUES ('set-a', 'chunk', 'rev-a', 'section', 'digest', 512, 0, 2048)",
    "INSERT INTO generations (collection_id, chunk_set_id, embedding_profile, sparse_profile)
     VALUES ('ctm', 'set-a', 'embed', 'bm25-en-fr/1')",
];

/// Rows holding a value their table's contract refuses; everything else in
/// them is valid.
const OUT_OF_CONTRACT: [&str; 14] = [
    // A status that is not a verdict of canonicalization.
    "INSERT INTO revisions (id, document_id, original_digest, canonical_digest, status,
       metadata_json)
     VALUES ('rev-b', 'doc-a', 'original', 'canonical', 'eligible', '{}')",
    // Metadata that is not a JSON object.
    "INSERT INTO revisions (id, document_id, original_digest, canonical_digest, status,
       metadata_json)
     VALUES ('rev-b', 'doc-a', 'original', 'canonical', 'valid', '[]')",
    // Profiles that are not a JSON object, a collection's and a source's.
    "INSERT INTO collections (id, title, visibility, profiles_json)
     VALUES ('other', 'Other', 'private', '[]')",
    "INSERT INTO sources (collection_id, id, kind, reference, profiles_json)
     VALUES ('ctm', 'other', 'import', 'manifest', '\"profile\"')",
    // A disposition that is not a quality outcome.
    "INSERT INTO quality_dispositions (revision_id, disposition, reasons_json, rule_ids,
       decided_by)
     VALUES ('rev-a', 'held', '[]', '[]', 'the gate')",
    // Reasons and rule IDs that are not JSON.
    "INSERT INTO quality_dispositions (revision_id, disposition, reasons_json, rule_ids,
       decided_by)
     VALUES ('rev-a', 'accepted', 'short', '[]', 'the gate')",
    "INSERT INTO quality_dispositions (revision_id, disposition, reasons_json, rule_ids,
       decided_by)
     VALUES ('rev-a', 'accepted', '[]', 'Q-001', 'the gate')",
    // A Jaccard similarity below zero, and one above one.
    "INSERT INTO near_dup_groups (group_id, revision_id, jaccard)
     VALUES ('group', 'rev-a', -0.1)",
    "INSERT INTO near_dup_groups (group_id, revision_id, jaccard)
     VALUES ('group', 'rev-a', 1.5)",
    // A negative token count, a span that starts before the text, and one that
    // ends before it starts.
    "INSERT INTO chunks (chunk_set_id, id, revision_id, digest, token_count, span_start,
       span_end)
     VALUES ('set-a', 'chunk', 'rev-a', 'digest', -1, 0, 1)",
    "INSERT INTO chunks (chunk_set_id, id, revision_id, digest, token_count, span_start,
       span_end)
     VALUES ('set-a', 'chunk', 'rev-a', 'digest', 1, -1, 1)",
    "INSERT INTO chunks (chunk_set_id, id, revision_id, digest, token_count, span_start,
       span_end)
     VALUES ('set-a', 'chunk', 'rev-a', 'digest', 1, 5, 4)",
    // A generation state outside its lifecycle, and a negative point count.
    "INSERT INTO generations (collection_id, chunk_set_id, embedding_profile, sparse_profile,
       state)
     VALUES ('ctm', 'set-a', 'embed', 'bm25-en-fr/1', 'verifying')",
    "INSERT INTO generations (collection_id, chunk_set_id, embedding_profile, sparse_profile,
       point_count)
     VALUES ('ctm', 'set-a', 'embed', 'bm25-en-fr/1', -1)",
];

/// The kernel database of `scratch` with the collection `ctm`, its source
/// `docs`, the document `doc-a`, its revision `rev-a` and the chunk set
/// `set-a` recorded.
fn parents(scratch: &Scratch) -> Database {
    let database = scratch.open();
    database
        .record_revision(&revision(&database, "rev-a", RevisionStatus::Valid))
        .unwrap();
    run(
        &database,
        "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
         VALUES ('set-a', 'ctm', 'profile', 'counter', 'complete')",
    )
    .unwrap();
    database
}

/// Runs `statement` on the writer and commits what it did.
fn run(database: &Database, statement: &str) -> rusqlite::Result<()> {
    database
        .write(|transaction| Ok::<_, Error>(transaction.execute_batch(statement)))
        .unwrap()
}

#[test]
fn the_migration_creates_the_ten_pipeline_tables_all_strict() {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(MIGRATION).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT name, strict FROM pragma_table_list
             WHERE schema = 'main' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .unwrap();
    let tables: Vec<(String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let expected: Vec<(String, i64)> = TABLES.iter().map(|name| ((*name).to_owned(), 1)).collect();
    assert_eq!(tables, expected);
}

#[test]
fn the_kernel_database_applies_the_documents_migration() {
    let scratch = Scratch::new();
    let database = scratch.empty();
    let applied: i64 = database
        .reader()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM migrations WHERE name = '0004_documents'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(applied, 1);
}

#[test]
fn every_pipeline_table_refuses_a_row_whose_parent_is_missing() {
    let scratch = Scratch::new();
    let database = parents(&scratch);
    for orphan in ORPHANS {
        assert_eq!(
            failure(run(&database, orphan)),
            Some(ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
            "{orphan}"
        );
    }
    for row in ROWS {
        assert_eq!(run(&database, row), Ok(()), "{row}");
    }
}

#[test]
fn every_pipeline_table_refuses_a_value_outside_its_contract() {
    let scratch = Scratch::new();
    let database = parents(&scratch);
    for row in OUT_OF_CONTRACT {
        assert_eq!(
            failure(run(&database, row)),
            Some(ffi::SQLITE_CONSTRAINT_CHECK),
            "{row}"
        );
    }
}
