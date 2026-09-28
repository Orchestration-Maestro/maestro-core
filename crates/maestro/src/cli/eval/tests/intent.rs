//! Intent expansion on the ladder: a `HyDE` rung's card reaches search and
//! ask, a card that cannot expand is refused, and reports count outcomes.

use super::{
    super::{
        engine::KernelEngine,
        reports::{PrivateRow, RungReport},
        runner::Engine as _,
    },
    reports::{BINARY, runs, to_json},
    support::{rung, suite},
};
use crate::{
    failure::Failure,
    knowledge::operations::{ask::tests::register_reasoning_answerer, tests::Scratch},
};
use maestro_kernel::{
    artifact::Digest,
    evidence::RouteStatus,
    gateway::{RouterClient, Url},
};
use maestro_knowledge::{index::Qdrant, search::IntentExpansion};
use serde_json::json;
use std::time::Duration;

/// An address where nothing listens.
const NOWHERE: &str = "http://127.0.0.1:1";

#[test]
fn a_hyde_rung_gives_its_card_to_search_and_ask_and_names_it() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, card) = register_reasoning_answerer(&kernel, b"intent", false);
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let mut engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();
    let mut off = rung("off");
    off.configuration.rerank = None;
    let (start, _) = engine.start(&off, &suite(0, 1)).unwrap();
    assert_eq!(start.intent, None);
    assert!(engine.search_context().unwrap().intent_expander.is_none());

    let mut hyde = off.clone();
    hyde.configuration.intent_expansion = IntentExpansion::Hyde;
    hyde.configuration.intent_card = Some(card.digest().as_str().to_owned());
    let (start, _) = engine.start(&hyde, &suite(0, 1)).unwrap();
    assert_eq!(start.intent.as_deref(), Some(card.digest().as_str()));
    assert!(engine.search_context().unwrap().intent_expander.is_some());
    assert_eq!(engine.provenance(&hyde).unwrap(), start);
}

#[test]
fn a_hyde_rung_is_refused_a_card_that_cannot_expand() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, thinking) = register_reasoning_answerer(&kernel, b"thinking", true);
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let mut engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();
    let unregistered = Digest::of(b"never registered");
    for (card, refusal) in [
        (thinking.digest().as_str(), "thinks"),
        (unregistered.as_str(), "not registered"),
    ] {
        let mut hyde = rung("hyde");
        hyde.configuration.rerank = None;
        hyde.configuration.intent_expansion = IntentExpansion::Hyde;
        hyde.configuration.intent_card = Some(card.to_owned());
        let refused = engine.start(&hyde, &suite(0, 1)).map(drop);
        assert!(
            matches!(&refused, Err(Failure::Refused(reason)) if reason.contains(refusal)),
            "{refused:?}"
        );
    }
}

#[test]
fn reports_count_each_intent_outcome_by_its_reason() {
    let mut runs = runs();
    let template = runs[0].diagnostics[0].clone();
    runs[0].diagnostics = [
        RouteStatus::Ok,
        RouteStatus::Unavailable("intent_not_triggered".to_owned()),
        RouteStatus::Unavailable("intent_guard_malformed".to_owned()),
        RouteStatus::Unavailable("intent_guard_protected_missing".to_owned()),
        RouteStatus::Unavailable("intent_guard_protected_missing".to_owned()),
        RouteStatus::Unavailable("intent_second_pass_unavailable".to_owned()),
        RouteStatus::Unavailable("intent_deadline_exceeded".to_owned()),
        RouteStatus::Unavailable("intent_model_unavailable".to_owned()),
    ]
    .into_iter()
    .map(|status| {
        let mut diagnostic = template.clone();
        diagnostic.intent_status = Some(status);
        diagnostic
    })
    .collect();
    let digest = Digest::of(b"suite");
    let report = RungReport::new(&runs[0], "docs", &digest, BINARY);
    assert_eq!(
        to_json(&report)["intent_outcomes"],
        json!({
            "ran": 1,
            "not_triggered": 1,
            "guarded_malformed": 1,
            "guarded_protected_missing": 2,
            "second_pass_unavailable": 1,
            "timeout": 1,
            "unavailable": 1,
        })
    );
    assert!(report.to_markdown().contains("- Intent outcomes: "));
    let off = RungReport::new(&runs[1], "docs", &digest, BINARY);
    assert!(to_json(&off).get("intent_outcomes").is_none());
    assert!(!off.to_markdown().contains("Intent outcomes"));
    let row = to_json(&PrivateRow::new(
        &runs[0].rows[0],
        &runs[0].diagnostics[3],
        &[],
        None,
        false,
    ));
    assert_eq!(
        row["intent_status"],
        json!({"unavailable": "intent_guard_protected_missing"})
    );
}

#[test]
fn intent_off_private_rows_keep_their_bytes() {
    let runs = runs();
    let mut row = runs[0].rows[0].clone();
    row.search.elapsed = Duration::ZERO;
    row.ask.elapsed = Duration::ZERO;
    let actual = serde_json::to_string(&PrivateRow::new(
        &row,
        &runs[0].diagnostics[0],
        &[],
        None,
        false,
    ))
    .unwrap();
    assert_eq!(
        actual,
        concat!(
            "{\"id\":\"a0\",\"search\":\"ranked\",\"search_us\":0,",
            "\"ranked_documents\":[\"doc-a0\"],\"expected_rank\":1,",
            "\"delivered\":[{\"source_ref\":\"doc:doc-a0\",\"doc_id\":\"doc-a0\",",
            "\"revision_id\":\"rev\",\"section_id\":\"section-a0\",\"span\":[0,10],",
            "\"digest\":\"sha256:",
            "0000000000000000000000000000000000000000000000000000000000000000\"}],",
            "\"bundle_documents\":[\"doc-a0\"],\"candidate_source_load_micros\":0,",
            "\"candidate_context_fallbacks\":[],\"bundle_rank\":1,",
            "\"top_rerank_score\":0.75,\"top_fused_score\":0.05,",
            "\"ask\":null,\"ask_us\":null,\"refusal\":null,",
            "\"citations\":[{\"document_id\":\"doc-a0\",\"revision_id\":\"rev\",",
            "\"chunk_id\":null,\"section_id\":\"section-a0\",\"span\":null}],\"rejections\":[],",
            "\"reply_cap\":null}"
        )
    );
    let mut expanded = runs[0].diagnostics[0].clone();
    expanded.intent_displaced = Some(2);
    let row = to_json(&PrivateRow::new(&row, &expanded, &[], None, false));
    assert_eq!(row["intent_displaced"], 2);
}
