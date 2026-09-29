//! Real search admission and fusion with a controlled expansion model.

use super::{
    configured_search::{Published, clean, published},
    intent_port::{IntentPort, run_configured},
};
use maestro_kernel::evidence::RouteStatus;
use maestro_knowledge::search::{
    EvidenceInput, IntentExpansion, IntentTrigger, Route, SearchConfiguration,
};
use std::sync::atomic::Ordering;
use tokio::time::Duration;

async fn run(fixture: &Published, port: &IntentPort<'_>, mode: IntentExpansion) -> EvidenceInput {
    run_configured(
        fixture,
        port,
        "scheduler job",
        SearchConfiguration {
            intent_expansion: mode,
            intent_deadline_ms: 100,
            rerank_enabled: false,
            ..SearchConfiguration::default()
        },
    )
    .await
}

#[tokio::test]
async fn off_never_calls_chat_and_hyde_adds_routes_without_replacing_originals() {
    let fixture = published().await;
    let port = IntentPort::new(
        &fixture.port,
        Some(r#"{"passage":"scheduler job definition","keywords":"scheduler"}"#),
    );
    let baseline = run(&fixture, &port, IntentExpansion::Off).await;
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    assert!(!baseline.routes.contains_key("intent_expansion"));
    let expanded = run(&fixture, &port, IntentExpansion::Hyde).await;
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    assert_eq!(expanded.understood, baseline.understood);
    assert_eq!(expanded.routes["intent_expansion"], RouteStatus::Ok);
    for route in [Route::Dense, Route::Lexical, Route::Identifier] {
        assert_eq!(
            expanded.observations.route_ranks[&route],
            baseline.observations.route_ranks[&route]
        );
    }
    for route in [Route::DenseIntent, Route::LexicalIntent] {
        assert!(!expanded.observations.route_ranks[&route].is_empty());
        assert!(
            expanded
                .ranked
                .iter()
                .any(|ranked| ranked.candidate.fused.ranks.contains_key(&route))
        );
        assert_eq!(expanded.routes[route.name()], RouteStatus::Ok);
    }
    clean(&fixture).await;
}

#[tokio::test]
async fn failed_unsafe_and_timed_out_expansion_keep_original_results() {
    let fixture = published().await;
    let mut port = IntentPort::new(&fixture.port, None);
    let baseline = run(&fixture, &port, IntentExpansion::Off).await;
    for (reply, delay, reason) in [
        (None, Duration::ZERO, "intent_model_unavailable"),
        (Some("not json"), Duration::ZERO, "intent_guard_malformed"),
        (
            Some(r#"{"passage":"scheduler job 42","keywords":"scheduler"}"#),
            Duration::ZERO,
            "intent_guard_added_number",
        ),
        (None, Duration::from_secs(1), "intent_deadline_exceeded"),
    ] {
        port.reply = reply;
        port.delay = delay;
        let fallback = run(&fixture, &port, IntentExpansion::Hyde).await;
        assert_eq!(fallback.observations, baseline.observations);
        assert_eq!(fallback.ranked, baseline.ranked);
        assert_eq!(
            fallback.routes["intent_expansion"],
            RouteStatus::Unavailable(reason.to_owned())
        );
        assert!(!fallback.routes.contains_key("dense_intent"));
    }
    clean(&fixture).await;
}

#[tokio::test]
async fn low_confidence_uses_one_original_pass_and_only_reranks_after_actual_expansion() {
    let fixture = published().await;
    let mut port = IntentPort::new(
        &fixture.port,
        Some(r#"{"passage":"scheduler job definition","keywords":["scheduler"]}"#),
    );
    for threshold in [0.0, 1.0, 2.0] {
        let configuration = SearchConfiguration {
            intent_expansion: IntentExpansion::Hyde,
            intent_trigger: IntentTrigger::LowConfidence {
                min_top_rerank: threshold,
            },
            ..SearchConfiguration::default()
        };
        let result = run_configured(&fixture, &port, "scheduler job", configuration).await;
        assert_eq!(
            result.routes["intent_expansion"],
            if threshold > 1.0 {
                RouteStatus::Ok
            } else {
                RouteStatus::Unavailable("intent_not_triggered".to_owned())
            }
        );
    }
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    // The first pass scored every candidate, so the second pass reused
    // those scores and called the reranker no more.
    assert_eq!(port.rerank_calls.load(Ordering::SeqCst), 3);
    port.reply = Some("invalid json");
    let fallback = run_configured(
        &fixture,
        &port,
        "scheduler job",
        SearchConfiguration {
            intent_expansion: IntentExpansion::Hyde,
            intent_trigger: IntentTrigger::LowConfidence {
                min_top_rerank: 2.0,
            },
            ..SearchConfiguration::default()
        },
    )
    .await;
    assert_eq!(port.rerank_calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        fallback.routes["intent_expansion"],
        RouteStatus::Unavailable("intent_guard_malformed".to_owned())
    );
    assert!(
        fallback
            .ranked
            .iter()
            .all(|ranked| ranked.score == Some(1.0))
    );
    clean(&fixture).await;
}

#[tokio::test]
async fn intent_runs_terminology_route_without_original_word_overlap_or_a_score() {
    let fixture = published().await;
    let port = IntentPort::new(
        &fixture.port,
        Some(r#"{"passage":"scheduler job definition","keywords":["scheduler"]}"#),
    );
    let result = run_configured(
        &fixture,
        &port,
        "repeat unsuccessful task",
        SearchConfiguration {
            dense_enabled: false,
            rerank_enabled: false,
            intent_expansion: IntentExpansion::Hyde,
            intent_trigger: IntentTrigger::LowConfidence {
                min_top_rerank: 1.0,
            },
            ..SearchConfiguration::default()
        },
    )
    .await;
    // The shared sparse fake returns zero-score hits too; prove vocabulary
    // separation from the actual candidate text instead of assuming it drops them.
    assert!(result.ranked.iter().all(|ranked| {
        ["repeat", "unsuccessful", "task"]
            .iter()
            .all(|word| !ranked.candidate.text.to_lowercase().contains(word))
    }));
    assert!(!result.observations.route_ranks[&Route::LexicalIntent].is_empty());
    assert!(!result.ranked.is_empty());
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    clean(&fixture).await;
}

#[tokio::test]
async fn low_confidence_keeps_original_ranking_when_expanded_routes_have_no_hits() {
    let fixture = published().await;
    let port = IntentPort::new(
        &fixture.port,
        Some(r#"{"passage":"Scheduler documentation","keywords":"!!!"}"#),
    );
    let result = run_configured(
        &fixture,
        &port,
        "scheduler job",
        SearchConfiguration {
            dense_enabled: false,
            intent_expansion: IntentExpansion::Hyde,
            intent_trigger: IntentTrigger::LowConfidence {
                min_top_rerank: 2.0,
            },
            ..SearchConfiguration::default()
        },
    )
    .await;
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    assert_eq!(port.rerank_calls.load(Ordering::SeqCst), 1);
    assert!(!result.ranked.is_empty());
    assert!(result.ranked.iter().all(|ranked| ranked.score == Some(1.0)));
    assert_eq!(result.routes["intent_expansion"], RouteStatus::Ok);
    for route in [Route::DenseIntent, Route::LexicalIntent] {
        assert!(result.routes.contains_key(route.name()));
        assert!(result.observations.route_ranks[&route].is_empty());
    }
    clean(&fixture).await;
}
