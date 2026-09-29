//! Pure route helpers: Qdrant scope filters and ranked-hit cleanup.

use maestro_kernel::{
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use std::{fs, path::PathBuf};

use crate::{
    index::ProjectionFilter,
    search::{
        filter::{query_filter, scope_filter},
        routes::results::{ScoredChunk, deduplicate, rank},
    },
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn scopes() -> (Scratch, Database, ScopeSet) {
    let scratch = Scratch::new();
    let database = Database::open_in(&scratch.0).unwrap();
    let scope: Scope = "workspace/default/collection/ctm".parse().unwrap();
    database
        .grant("reader", &scope, Right::Read, "test")
        .unwrap();
    let scopes = database.visible("reader").unwrap();
    (scratch, database, scopes)
}

#[test]
fn scope_filter_matches_any_granted_scope_tag() {
    let (_scratch, _database, scopes) = scopes();
    let filter = scope_filter(&scopes);
    let ProjectionFilter::AnyString { field, values } = filter else {
        panic!("scope filter should match granted string-array values: {filter:?}");
    };
    assert_eq!(field, "scope_tags");
    assert_eq!(values, ["workspace/default/collection/ctm"]);
}

#[test]
fn search_filter_adds_exact_version_after_scope_authorization() {
    let (_scratch, _database, scopes) = scopes();
    let filter = query_filter(&scopes, Some("v1.2"));
    let ProjectionFilter::All(filters) = filter else {
        panic!("version filter should combine scope and exact version: {filter:?}");
    };
    assert_eq!(filters.len(), 2);
    assert!(
        matches!(&filters[0], ProjectionFilter::AnyString { field, .. } if field == "scope_tags")
    );
    let ProjectionFilter::ExactString { field, value } = &filters[1] else {
        panic!(
            "version filter should match one exact string: {:?}",
            filters[1]
        );
    };
    assert_eq!(field, "version");
    assert_eq!(value, "v1.2");
}

#[test]
fn tied_hits_rank_by_chunk_id_after_score() {
    let hits = rank(
        vec![hit("z", "z", 0.9), hit("a", "a", 0.9), hit("b", "b", 0.8)],
        3,
    );
    assert_eq!(
        hits,
        vec![hit("a", "a", 0.9), hit("z", "z", 0.9), hit("b", "b", 0.8)]
    );
}

#[test]
fn ranked_hits_keep_the_first_duplicate_and_stop_at_k() {
    let hits = vec![
        hit("a", "first", 0.9),
        hit("a", "later", 0.8),
        hit("b", "second", 0.7),
        hit("c", "outside-k", 0.6),
    ];
    assert_eq!(
        deduplicate(hits, 2),
        vec![hit("a", "first", 0.9), hit("b", "second", 0.7)]
    );
    assert!(deduplicate(vec![hit("a", "first", 1.0)], 0).is_empty());
}

fn hit(chunk_id: &str, revision_id: &str, score: f64) -> ScoredChunk {
    ScoredChunk {
        chunk_id: chunk_id.to_owned(),
        revision_id: revision_id.to_owned(),
        score,
    }
}
