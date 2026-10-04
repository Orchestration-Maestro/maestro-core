//! Exact window boundaries, UTF-8 advancement and frozen model inputs.

use super::{
    DeterministicExtractor, GraphExtractor, ModelExtractor, Window, policy, source, test_card,
};
use crate::graph::{
    extract::{WindowPolicy, locate_quote, windows},
    verify::Source,
};
use maestro_canonicalization::SourceSpan;
use maestro_kernel::{artifact::Digest, gateway::extraction_prompt_digest};
use serde_json::json;
use std::fs;

#[test]
fn each_zero_window_bound_is_refused_and_displayed() {
    for (schema, bytes, count) in [
        ("maestro-graph-window-policy/1", 0, 4),
        ("maestro-graph-window-policy/1", 64, 0),
        ("maestro-graph-window-policy/2", 64, 4),
    ] {
        let error = WindowPolicy::parse(
            &json!({
                "schema":schema, "max_window_bytes":bytes,
                "overlap_bytes":0, "max_windows":count
            })
            .to_string(),
        )
        .map_err(|error| error.to_string());
        assert_eq!(error, Err("invalid window policy bounds".to_owned()));
    }
}

#[test]
fn invalid_and_empty_block_spans_are_both_refused() {
    let original = source("abc");
    for span in [
        SourceSpan { start: 0, end: 4 },
        SourceSpan { start: 1, end: 1 },
    ] {
        let mut canonical = original.canonical().clone();
        assert_eq!(canonical.blocks.len(), 1);
        canonical.blocks[0].source_spans = vec![span];
        let malformed = Source::new(original.revision_id().to_owned(), canonical, "abc".into());
        assert_eq!(
            windows(&malformed, &policy(4, 0, 8)),
            Err("invalid source block span")
        );
    }
}

#[test]
fn utf8_windows_adjust_both_end_and_overlap_to_exact_boundaries() {
    let original = source("aébcdef");
    let result = windows(&original, &policy(4, 2, 8)).unwrap();
    assert_eq!(
        result
            .iter()
            .map(|w| (w.span.start, w.span.end, w.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, 4, "aéb"), (1, 5, "ébc"), (3, 7, "bcde"), (5, 8, "def")]
    );
    let original = source("abéc");
    let result = windows(&original, &policy(3, 0, 8)).unwrap();
    assert_eq!(
        result
            .iter()
            .map(|w| (w.span.start, w.span.end, w.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, 2, "ab"), (2, 5, "éc")]
    );
    assert_eq!(
        windows(&source("é"), &policy(1, 0, 8)),
        Err("window policy splits a UTF-8 character")
    );
    assert_eq!(
        windows(&source("éabc"), &policy(3, 2, 8)),
        Err("window overlap prevents progress")
    );
}

#[test]
fn quote_search_advances_across_multibyte_characters() {
    let window = Window {
        block_id: "block".into(),
        span: SourceSpan { start: 10, end: 16 },
        text: "ééé".into(),
    };
    assert_eq!(locate_quote(&window, "éé"), Err("ambiguous quote"));
    assert_eq!(
        locate_quote(&window, "ééé"),
        Ok(SourceSpan { start: 10, end: 16 })
    );
    let followed = Window {
        text: "éz".into(),
        span: SourceSpan { start: 10, end: 13 },
        ..window
    };
    assert_eq!(
        locate_quote(&followed, "é"),
        Ok(SourceSpan { start: 10, end: 12 })
    );
}

#[test]
fn model_budget_and_all_job_identity_inputs_are_frozen() {
    let (path, card) = test_card();
    let card_digest = card.digest().clone();
    let policy_digest = Digest::of(b"bounded policy");
    let extractor = ModelExtractor::new(
        DeterministicExtractor {
            candidates: vec![],
            refuse: false,
        },
        card,
        policy(4, 0, 8),
        policy_digest.clone(),
        3075,
    )
    .unwrap();
    assert_eq!(extractor.token_budget(), Some(3075));
    assert_eq!(
        extractor.job_inputs(),
        Some(json!({
            "card_digest":card_digest.as_str(), "prompt_digest":extraction_prompt_digest().as_str(),
            "window_policy_digest":policy_digest.as_str(),
            "profile_digest":extractor.provenance().profile.as_str(), "token_budget":3075
        }))
    );
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn token_estimate_continues_at_budget_and_stops_only_after_exceeding_it() {
    let original = source("abcdefghijklmnop");
    assert_eq!(windows(&original, &policy(4, 0, 8)).unwrap().len(), 4);
    for (budget, expected) in [(1025, 2050), (1026, 2050), (1024, 1025)] {
        let (path, card) = test_card();
        let extractor = ModelExtractor::new(
            DeterministicExtractor {
                candidates: vec![],
                refuse: false,
            },
            card,
            policy(4, 0, 8),
            Digest::of(b"policy"),
            budget,
        )
        .unwrap();
        assert_eq!(
            extractor.estimated_tokens(&original),
            Ok(expected),
            "budget {budget}"
        );
        fs::remove_dir_all(path).unwrap();
    }
}
