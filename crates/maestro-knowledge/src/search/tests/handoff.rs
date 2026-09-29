//! Route joining and rerank handoff boundaries.

use super::{
    rerank::{FakePort, candidate, card},
    support::CandidateDb,
};
use crate::{
    index::{ProjectionError, Qdrant},
    query::understand,
    search::{
        DISABLED_BY_CONFIGURATION, SearchContext, SearchError,
        candidates::{self, Failure as CandidateFailure, check_control, classify_read},
        rerank::Reranker,
        rerank::rerank_candidates,
        route_execution::{join_route_futures, route_error_reason},
        routes::{
            error::RouteError,
            outcome::{IdentifierOutcome, RouteOutcome, StructuredOutcome},
        },
    },
};
use maestro_kernel::{
    evidence::RouteStatus,
    gateway::Role,
    retrieval::{Error as RetrievalError, ReadControl},
    store::Error as StoreError,
};
use rusqlite::Error as SqliteError;
use std::{
    error::Error as _,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration as StdDuration, Instant as StdInstant},
};
use tokio::{
    sync::Barrier,
    time::{Duration, Instant, timeout},
};

#[tokio::test]
async fn rerank_handoff_passes_eighty_of_one_hundred_twenty_candidates() {
    let port = FakePort::scores((0..80).map(f64::from).collect());
    let card = card(Role::Reranker, 128);
    let candidates = (0..120)
        .map(|index| {
            let id = format!("candidate-{index:03}");
            candidate(&id, 0.25, &id)
        })
        .collect();
    let (ranked, status) = rerank_candidates(
        &understand("query"),
        candidates,
        Some(&Reranker {
            port: &port,
            card: &card,
        }),
        Some(NonZeroUsize::new(80).unwrap()),
        Instant::now() + Duration::from_secs(2),
    )
    .await;

    assert_eq!(status, RouteStatus::Ok);
    assert_eq!(ranked.len(), 120);
    assert_eq!(port.calls.lock().unwrap()[0].documents.len(), 80);
    assert!(ranked[..80].iter().all(|item| item.score.is_some()));
    assert_eq!(
        ranked[80..]
            .iter()
            .map(|item| item.candidate.fused.chunk_id.as_str())
            .collect::<Vec<_>>(),
        (80..120)
            .map(|index| format!("candidate-{index:03}"))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn disabled_reranking_keeps_fused_order_without_calling_the_model() {
    let port = FakePort::scores(vec![1.0]);
    let card = card(Role::Reranker, 128);
    let (ranked, status) = rerank_candidates(
        &understand("query"),
        vec![candidate("candidate", 1.0, "prepared text")],
        Some(&Reranker {
            port: &port,
            card: &card,
        }),
        None,
        Instant::now() + Duration::from_secs(2),
    )
    .await;
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].score, None);
    assert_eq!(
        status,
        RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned())
    );
    assert!(port.calls.lock().unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn rerank_handoff_uses_only_the_remaining_request_time() {
    let port = FakePort::delayed_scores(vec![1.0], Duration::from_millis(20));
    let card = card(Role::Reranker, 128);
    let deadline = Instant::now() + Duration::from_millis(10);
    let (ranked, status) = rerank_candidates(
        &understand("query"),
        vec![candidate("candidate", 1.0, "prepared text")],
        Some(&Reranker {
            port: &port,
            card: &card,
        }),
        Some(NonZeroUsize::new(1).unwrap()),
        deadline,
    )
    .await;

    assert_eq!(
        status,
        RouteStatus::Unavailable("deadline_exceeded".to_owned())
    );
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].score, None);
}

#[test]
fn candidate_control_stops_for_deadline_or_cancellation_independently() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = ReadControl {
        deadline: StdInstant::now() + StdDuration::from_secs(1),
        cancelled: cancelled.clone(),
    };
    assert!(check_control(&control).is_ok());
    cancelled.store(true, Ordering::Relaxed);
    assert!(matches!(
        check_control(&control),
        Err(CandidateFailure::TimedOut)
    ));

    let elapsed = ReadControl {
        deadline: StdInstant::now()
            .checked_sub(StdDuration::from_secs(1))
            .unwrap_or_else(StdInstant::now),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    assert!(matches!(
        check_control(&elapsed),
        Err(CandidateFailure::TimedOut)
    ));
}

#[test]
fn search_context_debug_redacts_its_database_connection() {
    let database = CandidateDb::new(b"prepared text", "docs");
    let qdrant = Qdrant::new("http://127.0.0.1:6334").unwrap();
    let context = SearchContext::<()> {
        intent_expander: None,
        database: database.database.clone(),
        principal: "reader",
        projection: &qdrant,
        embedder: None,
        reranker: None,
        source_classes: None,
    };

    let debug = format!("{context:?}");
    assert!(debug.contains("database: \"Database\""));
    assert!(debug.contains("principal: \"reader\""));
    assert!(!debug.contains("writer"));
}

#[test]
fn search_errors_keep_safe_display_and_kernel_source() {
    let invalid = SearchError::InvalidRequest {
        reason: "query must not be blank".to_owned(),
    };
    assert_eq!(
        invalid.to_string(),
        "invalid search request: query must not be blank"
    );
    assert!(invalid.source().is_none());

    let kernel = SearchError::Kernel(RetrievalError::TimedOut);
    assert_eq!(kernel.to_string(), "search kernel operation failed");
    assert_eq!(
        kernel.source().unwrap().to_string(),
        "search read reached its deadline"
    );
}

#[test]
fn sqlite_store_failures_remain_kernel_errors_not_evidence_corruption() {
    let error = RetrievalError::Store(StoreError::Sqlite(SqliteError::InvalidQuery));
    assert!(matches!(classify_read(error), CandidateFailure::Kernel(_)));
}

#[tokio::test]
async fn candidate_loading_refuses_each_untrusted_kernel_claim_before_rerank() {
    let mismatched = CandidateDb::new(b"prepared text", "docs");
    let result = candidates::load(
        mismatched.database.clone(),
        mismatched.request(&mismatched.chunk_id, vec!["foreign-revision".to_owned()]),
    )
    .await;
    assert!(matches!(result, Err(CandidateFailure::EvidenceLoad)));

    let wrong_set = CandidateDb::new(b"prepared text", "docs");
    let result = candidates::load(
        wrong_set.database.clone(),
        wrong_set.request(
            "chunk-from-another-set",
            vec![wrong_set.revision_id.clone()],
        ),
    )
    .await;
    assert!(matches!(result, Err(CandidateFailure::EvidenceLoad)));

    let out_of_scope = CandidateDb::new(b"prepared text", "private");
    let result = candidates::load(
        out_of_scope.database.clone(),
        out_of_scope.request(
            &out_of_scope.chunk_id,
            vec![out_of_scope.revision_id.clone()],
        ),
    )
    .await;
    assert!(matches!(result, Err(CandidateFailure::EvidenceLoad)));
}

#[tokio::test]
async fn candidate_loading_refuses_corrupt_artifacts_and_invalid_utf8() {
    let corrupt = CandidateDb::new(b"prepared text", "docs");
    corrupt.corrupt_artifact();
    let result = candidates::load(
        corrupt.database.clone(),
        corrupt.request(&corrupt.chunk_id, vec![corrupt.revision_id.clone()]),
    )
    .await;
    assert!(matches!(result, Err(CandidateFailure::EvidenceLoad)));

    let invalid_utf8 = CandidateDb::new(&[0xff], "docs");
    let result = candidates::load(
        invalid_utf8.database.clone(),
        invalid_utf8.request(
            &invalid_utf8.chunk_id,
            vec![invalid_utf8.revision_id.clone()],
        ),
    )
    .await;
    assert!(matches!(result, Err(CandidateFailure::EvidenceLoad)));
}

#[test]
fn route_errors_keep_fixed_failure_categories() {
    assert_eq!(
        route_error_reason(&RouteError::EmbedderUnavailable {
            reason: "private port details".to_owned(),
        }),
        "embedder unavailable"
    );
    assert_eq!(
        route_error_reason(&RouteError::ProfileMismatch {
            expected: "expected".to_owned(),
            found: "found".to_owned(),
        }),
        "search profile mismatch"
    );
    assert_eq!(
        route_error_reason(&RouteError::Projection(ProjectionError::new(
            "Qdrant's answer is not what was asked for: private response details",
        ))),
        "Qdrant search failed"
    );
}

#[tokio::test]
async fn all_route_futures_start_together_and_one_failure_keeps_the_others() {
    let barrier = Arc::new(Barrier::new(4));
    let dense_barrier = barrier.clone();
    let lexical_barrier = barrier.clone();
    let identifier_barrier = barrier.clone();
    let structured_barrier = barrier;
    let outcomes = timeout(
        Duration::from_secs(1),
        join_route_futures(
            async move {
                dense_barrier.wait().await;
                unavailable("dense unavailable")
            },
            async move {
                lexical_barrier.wait().await;
                available()
            },
            async move {
                identifier_barrier.wait().await;
                IdentifierOutcome {
                    route: available(),
                    dropped: Vec::new(),
                }
            },
            async move {
                structured_barrier.wait().await;
                Some(StructuredOutcome {
                    route: available(),
                    inventory: None,
                })
            },
        ),
    )
    .await
    .expect("every route must be polled concurrently");

    assert_eq!(
        outcomes.0.status,
        RouteStatus::Unavailable("dense unavailable".to_owned())
    );
    assert_eq!(outcomes.1.status, RouteStatus::Ok);
    assert_eq!(outcomes.2.route.status, RouteStatus::Ok);
    assert_eq!(outcomes.3.unwrap().route.status, RouteStatus::Ok);
}

fn available() -> RouteOutcome {
    RouteOutcome {
        hits: Vec::new(),
        status: RouteStatus::Ok,
    }
}

fn unavailable(reason: &str) -> RouteOutcome {
    RouteOutcome {
        hits: Vec::new(),
        status: RouteStatus::Unavailable(reason.to_owned()),
    }
}
