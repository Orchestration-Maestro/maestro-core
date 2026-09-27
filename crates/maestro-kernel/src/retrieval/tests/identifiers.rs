//! Controlled exact-index identifier reads.

use super::support::SearchDb;
use crate::retrieval::{ChunkHit, Error, IdentifierSearchResult, ReadControl, SearchRead};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn identifier_hits_requires_readiness_then_returns_exact_matches() {
    let search = SearchDb::with_input_population(&["Install the tool with --force."], 20);
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
    let result = search
        .database
        .identifier_hits(&read, &identifiers, 10)
        .unwrap();
    assert_eq!(result.hits.len(), 1);
    assert!(!result.skipped_too_common);
    assert_eq!(
        result
            .hits
            .iter()
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
            .hits
            .is_empty()
    );
    assert!(
        search
            .database
            .identifier_hits(&read, &identifiers, 0)
            .unwrap()
            .hits
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
            .hits
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
fn exact_index_lookup_is_case_sensitive_and_does_not_need_fts() {
    let search = SearchDb::with_input_population(&["Install the tool with --force."], 20);
    search.ready();
    assert_eq!(search_hits(&search, &["--force"]).len(), 1);
    assert!(search_hits(&search, &["force"]).is_empty());
    assert!(search_hits(&search, &["--Force"]).is_empty());
}

#[test]
fn common_identifiers_over_ten_percent_are_skipped() {
    let mut inputs = vec!["unrelated entry"; 100];
    inputs[..11].fill("ctm appears in this prepared input");
    let search = SearchDb::with_inputs(&inputs);
    search.ready();
    let result = search_result(&search, &["ctm"]);
    assert!(result.hits.is_empty());
    assert!(result.skipped_too_common);
}

#[test]
fn identifiers_at_ten_percent_are_not_skipped() {
    let mut inputs = vec!["unrelated entry"; 100];
    inputs[..10].fill("ctm appears in this prepared input");
    let search = SearchDb::with_inputs(&inputs);
    search.ready();
    let result = search_result(&search, &["ctm"]);
    assert_eq!(result.hits.len(), 10);
    assert!(!result.skipped_too_common);
}

#[test]
fn too_common_terms_are_skipped_while_rare_terms_still_return_hits() {
    let mut inputs = vec!["unrelated entry"; 100];
    inputs[..11].fill("ctm appears in this prepared input");
    inputs[50] = "rare appears in this prepared input";
    let search = SearchDb::with_inputs(&inputs);
    search.ready();

    let result = search_result(&search, &["ctm", "rare"]);
    assert!(result.skipped_too_common);
    assert_eq!(
        result
            .hits
            .iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-50"]
    );
}

#[test]
fn exact_lookup_finds_a_late_identifier_in_a_large_chunk_set() {
    let mut inputs = vec!["unrelated entry"; 2_000];
    inputs[1_999] = "rare appears in this prepared input";
    let search = SearchDb::with_inputs(&inputs);
    search.ready();
    assert_eq!(
        search_hits_limit(&search, &["rare"], 1)
            .iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-1999"]
    );
}

#[test]
fn identifier_hits_return_stable_chunk_id_order() {
    let search = SearchDb::with_input_population(
        &[
            "rare appears among many unrelated words in this long prepared input",
            "rare",
        ],
        20,
    );
    search.ready();
    assert_eq!(
        search_hits(&search, &["rare"])
            .iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-a", "chunk-b"]
    );
}

#[test]
fn identifier_hits_stops_at_the_requested_unique_chunk_limit() {
    let search = SearchDb::with_input_population(
        &[
            "Install the tool with --force.",
            "Install the tool with --force again.",
        ],
        20,
    );
    search.ready();
    assert_eq!(search_hits_limit(&search, &["--force"], 1).len(), 1);
}

#[test]
fn multiple_identifiers_merge_deduplicate_and_keep_the_global_top_limit() {
    let search = SearchDb::with_input_population(
        &["Use --alpha.", "Use --beta.", "Use --alpha with --beta."],
        20,
    );
    search.ready();
    assert_eq!(
        search_hits_limit(&search, &["--alpha", "--beta"], 2)
            .iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-2", "chunk-a"]
    );
}

#[test]
fn bare_command_code_span_finds_a_plain_prepared_input() {
    let search = SearchDb::with_input_population(&["The ctm command repairs the local cache."], 20);
    search.ready();
    assert_eq!(search_hits(&search, &["ctm"]).len(), 1);
}

#[test]
fn exact_values_are_not_interpreted_as_query_syntax() {
    let search = SearchDb::with_input_population(&["The exact phrase is one OR two."], 20);
    search.ready();
    assert_eq!(search_hits(&search, &["one OR two"]).len(), 1);
    assert!(search_hits(&search, &["One OR two"]).is_empty());

    let quoted = SearchDb::with_input_population(&["Use the \"a\" option."], 20);
    quoted.ready();
    assert_eq!(search_hits(&quoted, &["\"a\""]).len(), 1);
}

#[test]
fn identifier_hits_rejects_limits_that_do_not_fit_sqlite() {
    let search = SearchDb::new("Install the tool with --force.");
    search.ready();
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
    assert!(matches!(
        search
            .database
            .identifier_hits(&read, &["--force".to_owned()], usize::MAX),
        Err(Error::TooLarge)
    ));
}

fn search_hits(search: &SearchDb, identifiers: &[&str]) -> Vec<ChunkHit> {
    search_hits_limit(search, identifiers, 20)
}

fn search_hits_limit(search: &SearchDb, identifiers: &[&str], limit: usize) -> Vec<ChunkHit> {
    search_result_limit(search, identifiers, limit).hits
}

fn search_result(search: &SearchDb, identifiers: &[&str]) -> IdentifierSearchResult {
    search_result_limit(search, identifiers, 20)
}

fn search_result_limit(
    search: &SearchDb,
    identifiers: &[&str],
    limit: usize,
) -> IdentifierSearchResult {
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
