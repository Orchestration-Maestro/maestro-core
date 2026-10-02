use super::super::identifiers::indexed_hits;
use super::clock::control;
use crate::{
    generation::{Generation, GenerationState},
    retrieval::SearchRead,
    scope::{Scope, ScopeSet},
};
use rusqlite::{Connection, TransactionBehavior};
use std::collections::BTreeSet;

#[test]
fn identifier_index_filters_scope_before_its_hit_limit() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE chunk_search_identifiers (
               chunk_set_id TEXT, identifier TEXT, chunk_id TEXT,
               PRIMARY KEY (chunk_set_id, identifier, chunk_id)
             ) WITHOUT ROWID;
             CREATE TABLE chunks (chunk_set_id TEXT, id TEXT, revision_id TEXT);
             CREATE TABLE revisions (
               id TEXT, document_id TEXT, status TEXT, metadata_json TEXT
             );
             CREATE TABLE documents (id TEXT, collection_id TEXT, source_id TEXT);
             CREATE TABLE quality_dispositions (revision_id TEXT, disposition TEXT);",
        )
        .unwrap();
    for (chunk, revision, document, source, disposition) in [
        (
            "aaa-forbidden",
            "rev-secret",
            "doc-secret",
            "secret",
            "accepted",
        ),
        (
            "bbb-ineligible",
            "rev-ineligible",
            "doc-ineligible",
            "docs",
            "rejected",
        ),
        (
            "zzz-allowed",
            "rev-visible",
            "doc-visible",
            "docs",
            "accepted",
        ),
    ] {
        connection
            .execute(
                "INSERT INTO chunk_search_identifiers VALUES ('set-a', 'ctm', ?1)",
                [chunk],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO chunks VALUES ('set-a', ?1, ?2)",
                rusqlite::params![chunk, revision],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO revisions VALUES (?1, ?2, 'valid', '{}')",
                rusqlite::params![revision, document],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO documents VALUES (?1, 'ctm', ?2)",
                rusqlite::params![document, source],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO quality_dispositions VALUES (?1, ?2)",
                rusqlite::params![revision, disposition],
            )
            .unwrap();
    }
    let scope: Scope = "workspace/default/collection/ctm/source/docs"
        .parse()
        .unwrap();
    let scopes = ScopeSet::new(BTreeSet::from([scope]));
    let generation = Generation {
        id: 1,
        collection_id: "ctm".to_owned(),
        chunk_set_id: "set-a".to_owned(),
        embedding_profile: "embed:test".to_owned(),
        sparse_profile: "bm25-en-fr/1".to_owned(),
        state: GenerationState::Building,
        point_count: None,
        published_at: None,
    };
    let control = control();
    let read = SearchRead {
        generation: &generation,
        scopes: &scopes,
        version: None,
        control: &control,
    };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .unwrap();
    let hits = indexed_hits(&transaction, &read, &["ctm".to_owned()], 1).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk_id, "zzz-allowed");
}
