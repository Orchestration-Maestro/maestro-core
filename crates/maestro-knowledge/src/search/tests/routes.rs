//! Pure route helpers: Qdrant scope filters and ranked-hit cleanup.

use maestro_kernel::{
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use qdrant_client::qdrant::{condition::ConditionOneOf, r#match::MatchValue};
use std::{
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::search::{
    filter::scope_filter,
    routes::results::{ScoredChunk, deduplicate},
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(env::temp_dir().join(format!(
            "maestro-search-routes-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
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
    let [condition] = filter.must.as_slice() else {
        panic!("the scope filter must have one Qdrant condition: {filter:?}");
    };
    let Some(ConditionOneOf::Field(field)) = condition.condition_one_of.as_ref() else {
        panic!("the scope filter must use a field condition: {condition:?}");
    };
    assert_eq!(field.key, "scope_tags");
    assert!(matches!(
        field.r#match.as_ref().and_then(|matched| matched.match_value.as_ref()),
        Some(MatchValue::Keywords(scopes)) if scopes.strings == ["workspace/default/collection/ctm"]
    ));
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
