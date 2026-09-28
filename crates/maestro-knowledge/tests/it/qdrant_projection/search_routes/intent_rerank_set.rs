//! What the rerank scores once intent routes voted: the original top-depth
//! always, the candidates the votes lifted, and nothing twice.

use super::{
    configured_search::{clean, published},
    intent_fallbacks::FILLER,
    intent_port::{IntentPort, run_configured},
};
use maestro_kernel::evidence::RouteStatus;
use maestro_knowledge::search::{IntentExpansion, IntentTrigger, Route, SearchConfiguration};
use std::{num::NonZeroUsize, sync::atomic::Ordering};

#[tokio::test]
async fn intent_votes_never_push_an_original_candidate_out_of_the_rerank() {
    let fixture = published().await;
    let port = IntentPort::new(&fixture.port, Some(FILLER));
    let configuration = SearchConfiguration {
        rerank_depth: NonZeroUsize::new(1).unwrap(),
        ..SearchConfiguration::default()
    };
    let off = run_configured(&fixture, &port, "scheduler job", configuration).await;
    let original = &off.ranked[0].candidate.fused.chunk_id;
    assert!(off.ranked[0].score.is_some());
    let expanded = run_configured(
        &fixture,
        &port,
        "scheduler job",
        SearchConfiguration {
            intent_expansion: IntentExpansion::Hyde,
            ..configuration
        },
    )
    .await;
    let kept = expanded
        .ranked
        .iter()
        .find(|ranked| &ranked.candidate.fused.chunk_id == original)
        .unwrap();
    assert!(kept.score.is_some(), "{original} was not scored");
    assert_eq!(expanded.observations.intent_displaced, Some(1));
    assert_eq!(off.observations.intent_displaced, None);
    // The intent's own first candidate is scored too.
    let added = &expanded.observations.route_ranks[&Route::DenseIntent][0];
    assert!(
        expanded
            .ranked
            .iter()
            .any(|ranked| &ranked.candidate.fused.chunk_id == added && ranked.score.is_some())
    );
    clean(&fixture).await;
}

#[tokio::test]
async fn low_confidence_scores_only_the_candidates_the_expansion_lifted() {
    let fixture = published().await;
    let port = IntentPort::new(&fixture.port, Some(FILLER));
    let result = run_configured(
        &fixture,
        &port,
        "scheduler job",
        SearchConfiguration {
            intent_expansion: IntentExpansion::Hyde,
            intent_trigger: IntentTrigger::LowConfidence {
                min_top_rerank: 2.0,
            },
            rerank_depth: NonZeroUsize::new(1).unwrap(),
            ..SearchConfiguration::default()
        },
    )
    .await;
    // The first pass scored its one candidate; the second pass sent the
    // reranker only the one the expansion lifted, and reused the first.
    assert_eq!(*port.reranked.lock().unwrap(), [1, 1]);
    assert_eq!(result.routes["intent_expansion"], RouteStatus::Ok);
    assert_eq!(result.routes["rerank"], RouteStatus::Ok);
    let scored = result
        .ranked
        .iter()
        .filter(|ranked| ranked.score.is_some())
        .map(|ranked| ranked.candidate.fused.chunk_id.as_str())
        .collect::<Vec<_>>();
    // Equal scores keep the combined fused order.
    let added = &result.observations.route_ranks[&Route::DenseIntent][0];
    assert_eq!(scored, [added.as_str(), "chunk-0-0"]);
    assert_eq!(result.observations.intent_displaced, Some(1));
    clean(&fixture).await;
}

#[tokio::test]
async fn low_confidence_judges_the_best_score_not_the_first() {
    let fixture = published().await;
    let mut port = IntentPort::new(&fixture.port, Some(FILLER));
    port.score_of = Some(|text| if text.contains("Retries") { 5.0 } else { 1.0 });
    let configuration = SearchConfiguration {
        // Fused order alone decides the final order.
        rerank_blend: Some(1.0),
        intent_expansion: IntentExpansion::Hyde,
        intent_trigger: IntentTrigger::LowConfidence {
            min_top_rerank: 2.0,
        },
        ..SearchConfiguration::default()
    };
    let result = run_configured(&fixture, &port, "scheduler job", configuration).await;
    assert_eq!(result.ranked[0].score, Some(1.0));
    assert!(result.ranked.iter().any(|ranked| ranked.score == Some(5.0)));
    assert_eq!(
        result.routes["intent_expansion"],
        RouteStatus::Unavailable("intent_not_triggered".to_owned())
    );
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    clean(&fixture).await;
}
