//! Controlled identifier-hit reads.

use super::support::SearchDb;
use crate::retrieval::{ChunkHit, Error, ReadControl, SearchRead};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn identifier_hits_requires_readiness_then_returns_literal_fts_matches() {
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
    let identifiers = vec!["--force".to_owned()];

    assert!(matches!(
        search.database.identifier_hits(&read, &identifiers, 10),
        Err(Error::ProjectionMissing)
    ));

    search.ready();
    let hits = search
        .database
        .identifier_hits(&read, &identifiers, 10)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits.iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-a"]
    );
}

#[test]
fn empty_identifier_calls_need_no_marker_and_sixty_four_ids_are_allowed() {
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
    let identifiers = vec!["--force".to_owned()];
    assert!(
        search
            .database
            .identifier_hits(&read, &[], 10)
            .unwrap()
            .is_empty()
    );
    assert!(
        search
            .database
            .identifier_hits(&read, &identifiers, 0)
            .unwrap()
            .is_empty()
    );

    search.ready();
    let sixty_four = (0..64)
        .map(|index| format!("missing-{index}"))
        .collect::<Vec<_>>();
    assert!(
        search
            .database
            .identifier_hits(&read, &sixty_four, 10)
            .unwrap()
            .is_empty()
    );
    let sixty_five = (0..65)
        .map(|index| format!("missing-{index}"))
        .collect::<Vec<_>>();
    assert!(matches!(
        search.database.identifier_hits(&read, &sixty_five, 10),
        Err(Error::TooLarge)
    ));
}

#[test]
fn fts_hits_keep_rank_then_chunk_id_order() {
    let search = SearchDb::with_inputs(&[
        "rare appears among many unrelated words in this long prepared input",
        "rare",
    ]);
    search.ready();
    assert_eq!(
        search_hits(&search, &["rare"])
            .iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-b", "chunk-a"]
    );
}

#[test]
fn identifier_hits_stops_at_the_requested_unique_chunk_limit() {
    let search = SearchDb::with_inputs(&[
        "Install the tool with --force.",
        "Install the tool with --force again.",
    ]);
    search.ready();
    assert_eq!(search_hits_limit(&search, &["--force"], 1).len(), 1);
}

#[test]
fn bare_command_code_span_finds_a_plain_prepared_input() {
    let search = SearchDb::new("The ctm command repairs the local cache.");
    search.ready();
    assert_eq!(search_hits(&search, &["ctm"]).len(), 1);
}

#[test]
fn punctuation_only_identifier_uses_the_scoped_input_scan() {
    let search = SearchDb::new("Run -- now.");
    search.ready();
    assert_eq!(search_hits(&search, &["--"]).len(), 1);
}

#[test]
fn fts_operators_are_literal_data_and_exact_case_is_rechecked() {
    let search = SearchDb::new("The exact phrase is one OR two.");
    search.ready();
    assert_eq!(search_hits(&search, &["one OR two"]).len(), 1);
    assert!(search_hits(&search, &["One OR two"]).is_empty());

    let quoted = SearchDb::new("Use the \"a\" option.");
    quoted.ready();
    assert_eq!(search_hits(&quoted, &["\"a\""]).len(), 1);
}

fn search_hits(search: &SearchDb, identifiers: &[&str]) -> Vec<ChunkHit> {
    search_hits_limit(search, identifiers, 20)
}

fn search_hits_limit(search: &SearchDb, identifiers: &[&str], limit: usize) -> Vec<ChunkHit> {
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
    let identifiers = identifiers
        .iter()
        .map(|identifier| (*identifier).to_owned())
        .collect::<Vec<_>>();
    search
        .database
        .identifier_hits(&read, &identifiers, limit)
        .unwrap()
}
