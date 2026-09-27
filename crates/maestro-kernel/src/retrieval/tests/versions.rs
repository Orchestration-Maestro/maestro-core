//! Exact version existence in a pinned, scoped generation.

use super::support::SearchDb;
use crate::retrieval::{ReadControl, SearchRead};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn version_exists_matches_only_the_exact_pinned_revision_version() {
    let search = SearchDb::new("Install the tool with --force.");
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = |version| SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version,
        control: &control,
    };

    assert!(search.database.version_exists(&read(Some("1.2"))).unwrap());
    assert!(!search.database.version_exists(&read(Some("9.9"))).unwrap());
    assert!(search.database.version_exists(&read(None)).unwrap());
}
