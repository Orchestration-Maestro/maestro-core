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
use tokio::time::{self, Instant};

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
    let searched = || async {
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
    let fused = StageEnd::watch("retrieval.fuse");
    let advance = async {
        match slow {
            SlowReranker::Loading => fused.ended().await,
            SlowReranker::Hanging => fixture.port.wait_for_rerank().await,
        }
        // Setup ends at 450 ms; Tokio rounds its timer up by 1 ms.
        // Keep the clock held afterwards, including through assembly.
        time::advance(Duration::from_millis(451)).await;
        future::pending::<()>().await;
    };
    let (left, bundle) = on_stopped_clock(advance, searched).await;
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
    let (rerank, calls, left, passages) = search_with_slow_reranker(SlowReranker::Loading).await;

    assert_eq!(
        rerank,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert_eq!(calls, 0);
    assert_eq!(left, Duration::from_millis(549));
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
    let searched = || async {
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
    let stages: &[&str] = match slow {
        SlowModels::Reranker => &["retrieval.fuse"],
        SlowModels::Embedder | SlowModels::Both => {
            &["retrieval.route.lexical", "retrieval.route.identifier"]
        }
    };
    let routes = StageEnd::watch_all(stages);
    let advance = async {
        routes.ended().await;
        // Routes end at 23.95 s; reranker setup ends at 26.95 s.
        // Advance only past the needed cutoff, never through assembly.
        let cutoff = match slow {
            SlowModels::Embedder => Duration::from_millis(23_951),
            SlowModels::Reranker | SlowModels::Both => Duration::from_millis(26_951),
        };
        time::advance(cutoff).await;
        future::pending::<()>().await;
    };
    let (left, bundle) = on_stopped_clock(advance, searched).await;
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
async fn warm_search_is_byte_deterministic() {
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
        let input = on_stopped_clock(future::pending(), || {
            Box::pin(search(&search_context, &request))
        })
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
