//! A cold or slow reranker costs a search only its rerank.
//!
//! The search returns the fused order within its deadline, with the rerank
//! reported unavailable.

use super::super::stopped_clock::{StageEnd, on_stopped_clock};
use super::configured_search::{clean, context, published};
use super::models::{self, SlowModels, SlowReranker};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::Role,
};
use maestro_knowledge::search::evidence::EvidenceSettings;
use maestro_knowledge::search::{
    DEADLINE_EXCEEDED, SearchConfiguration, SearchRequest,
    evidence::{EvidenceCounter, assemble_evidence},
    search,
};
use std::{future, time::Duration};
use tokio::time::Instant;

/// The search deadline of these tests: evidence assembly keeps two
/// assembly windows of 250 ms and the 50 ms reserve, 550 ms.
const DEADLINE: Duration = Duration::from_millis(1000);
/// The default search cap exercised with cold model preparation.
const COLD_DEADLINE: Duration = Duration::from_secs(30);

/// How a search with a reranker that is `slow` ended: the rerank status, the
/// reranking calls made, the time left before the deadline when the search
/// handed over to evidence assembly, and the passages assembled.
async fn search_with_slow_reranker(slow: SlowReranker) -> (RouteStatus, usize, Duration, usize) {
    let mut fixture = published().await;
    fixture.port = models::Embedder::with_slow_reranker(slow);
    let reranker_card = models::card(Role::Reranker, 3);
    let search_context = context(&fixture, Some(&reranker_card));
    let request = SearchRequest {
        evidence: EvidenceSettings::default(),
        collection: &fixture.kernel.collection,
        text: "scheduler",
        version: None,
        budget: RequestBudget {
            deadline_ms: u32::try_from(DEADLINE.as_millis()).unwrap(),
            ..RequestBudget::default()
        },
        configuration: SearchConfiguration::default(),
    };
    let searched = async {
        let started = Instant::now();
        let input = Box::pin(search(&search_context, &request)).await.unwrap();
        let left = input.deadline.saturating_duration_since(Instant::now());
        let bundle = assemble_evidence(
            fixture.kernel.database.clone(),
            input,
            EvidenceCounter::Utf8Bytes,
        )
        .await
        .unwrap();
        assert!(started.elapsed() < DEADLINE, "{:?}", started.elapsed());
        (left, bundle)
    };
    // The clock stands still until fusion ended, when the search waits on
    // the reranker and its cutoff alone; it then moves only to that cutoff,
    // so what a loaded host takes costs the search no time.
    let fused = StageEnd::watch("retrieval.fuse");
    let (left, bundle) = on_stopped_clock(fused.ended(), searched).await;
    let outcome = (
        bundle.routes["rerank"].clone(),
        fixture.port.rerank_calls(),
        left,
        bundle.passages.len(),
    );
    clean(&fixture).await;
    outcome
}

#[tokio::test]
async fn a_reranker_still_loading_is_skipped_and_the_search_keeps_its_deadline() {
    let (rerank, calls, _, passages) = search_with_slow_reranker(SlowReranker::Loading).await;

    assert_eq!(
        rerank,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert_eq!(calls, 0);
    assert!(passages > 0);
}

/// Runs a search with `slow` model preparation and assembles its evidence.
async fn search_with_cold_models(slow: SlowModels) -> (RouteStatus, RouteStatus, Duration, usize) {
    let mut fixture = published().await;
    fixture.port = models::Embedder::with_slow_models(slow);
    let reranker_card = models::card(Role::Reranker, 3);
    let search_context = context(&fixture, Some(&reranker_card));
    let request = SearchRequest {
        evidence: EvidenceSettings::default(),
        collection: &fixture.kernel.collection,
        text: "scheduler",
        version: None,
        budget: RequestBudget {
            deadline_ms: u32::try_from(COLD_DEADLINE.as_millis()).unwrap(),
            ..RequestBudget::default()
        },
        configuration: SearchConfiguration::default(),
    };
    let searched = async {
        let started = Instant::now();
        let input = Box::pin(search(&search_context, &request)).await.unwrap();
        let left = input.deadline.saturating_duration_since(Instant::now());
        let bundle = assemble_evidence(
            fixture.kernel.database.clone(),
            input,
            EvidenceCounter::Utf8Bytes,
        )
        .await
        .unwrap();
        assert!(started.elapsed() < COLD_DEADLINE, "{:?}", started.elapsed());
        (left, bundle)
    };
    let routes = StageEnd::watch_all(&["retrieval.route.lexical", "retrieval.route.identifier"]);
    let (left, bundle) = on_stopped_clock(routes.ended(), searched).await;
    let outcome = (
        bundle.routes["dense"].clone(),
        bundle.routes["rerank"].clone(),
        left,
        bundle.passages.len(),
    );
    clean(&fixture).await;
    outcome
}

#[tokio::test]
async fn an_embedder_still_loading_degrades_dense_and_returns_within_deadline() {
    let (dense, rerank, left, passages) = search_with_cold_models(SlowModels::Embedder).await;

    assert_eq!(
        dense,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert_eq!(rerank, RouteStatus::Ok);
    assert!(left >= Duration::from_secs(3), "{left:?}");
    assert!(passages > 0);
}

#[tokio::test]
async fn a_reranker_still_loading_degrades_rerank_and_returns_within_deadline() {
    let (dense, rerank, left, passages) = search_with_cold_models(SlowModels::Reranker).await;

    assert_eq!(dense, RouteStatus::Ok);
    assert_eq!(
        rerank,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert!(left >= Duration::from_secs(3), "{left:?}");
    assert!(passages > 0);
}

#[tokio::test]
async fn both_models_still_loading_degrade_only_their_routes_and_return_within_deadline() {
    let (dense, rerank, left, passages) = search_with_cold_models(SlowModels::Both).await;

    assert_eq!(
        dense,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert_eq!(
        rerank,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert!(left >= Duration::from_secs(3), "{left:?}");
    assert!(passages > 0);
}

#[tokio::test]
async fn repeated_warm_searches_return_byte_identical_bundles() {
    let fixture = published().await;
    let reranker_card = models::card(Role::Reranker, 3);
    let search_context = context(&fixture, Some(&reranker_card));
    let request = SearchRequest {
        evidence: EvidenceSettings::default(),
        collection: &fixture.kernel.collection,
        text: "scheduler",
        version: None,
        budget: RequestBudget {
            deadline_ms: u32::try_from(COLD_DEADLINE.as_millis()).unwrap(),
            ..RequestBudget::default()
        },
        configuration: SearchConfiguration::default(),
    };
    let mut serialized = Vec::new();
    for _ in 0..2 {
        let input = on_stopped_clock(
            future::pending(),
            Box::pin(search(&search_context, &request)),
        )
        .await
        .unwrap();
        let bundle = assemble_evidence(
            fixture.kernel.database.clone(),
            input,
            EvidenceCounter::Utf8Bytes,
        )
        .await
        .unwrap();
        serialized.push(serde_json::to_vec(&bundle).unwrap());
    }
    assert_eq!(serialized[0], serialized[1]);
    clean(&fixture).await;
}

#[tokio::test]
async fn a_reranker_that_never_scores_leaves_evidence_assembly_its_time() {
    let (rerank, calls, left, passages) = search_with_slow_reranker(SlowReranker::Hanging).await;

    assert_eq!(
        rerank,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert_eq!(calls, 1);
    assert!(left >= Duration::from_millis(450), "{left:?}");
    assert!(passages > 0);
}
