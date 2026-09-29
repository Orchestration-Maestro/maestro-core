//! An expanded search never ends worse than the original one: a failed or
//! slow second pass, a small budget and revoked rights, through `search`.

use super::super::stopped_clock::{StageEnd, on_stopped_clock};
use super::{
    configured_search::{clean, published},
    intent_port::{IntentPort, RerankFault, run_configured, search_with},
};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    scope::Right,
};
use maestro_knowledge::search::{
    IntentExpansion, IntentTrigger, Route, SearchConfiguration, SearchError,
};
use std::{future, num::NonZeroUsize, sync::atomic::Ordering};
use tokio::{
    task::yield_now,
    time::{Duration, Instant},
};

/// A reply whose passage and keywords find the filler chunks first.
pub(super) const FILLER: &str =
    r#"{"passage":"unrelated frequency padding","keywords":"unrelated frequency padding"}"#;

/// Waits for the search to begin its second rerank call.
async fn second_rerank_started(port: &IntentPort<'_>) {
    while port.rerank_calls.load(Ordering::SeqCst) < 2 {
        yield_now().await;
    }
}

/// Waits for the first intent-chat request.
async fn chat_started(port: &IntentPort<'_>) {
    while port.calls.load(Ordering::SeqCst) == 0 {
        yield_now().await;
    }
}

/// The 1.5 s budget a caller may pass.
fn small_budget() -> RequestBudget {
    RequestBudget {
        deadline_ms: 1500,
        ..RequestBudget::default()
    }
}

/// Low-confidence expansion that always fires, reranking one candidate.
fn low_confidence(mode: IntentExpansion) -> SearchConfiguration {
    SearchConfiguration {
        intent_expansion: mode,
        intent_trigger: IntentTrigger::LowConfidence {
            min_top_rerank: 2.0,
        },
        rerank_depth: NonZeroUsize::new(1).unwrap(),
        ..SearchConfiguration::default()
    }
}

#[tokio::test]
async fn a_failed_or_hanging_second_rerank_keeps_the_first_ranking() {
    let fixture = published().await;
    let off = IntentPort::new(&fixture.port, Some(FILLER));
    let first = on_stopped_clock(
        future::pending(),
        Box::pin(search_with(
            &fixture,
            &off,
            "scheduler job",
            low_confidence(IntentExpansion::Off),
            small_budget(),
        )),
    )
    .await
    .unwrap();
    for fault in [RerankFault::Fails, RerankFault::Hangs] {
        let mut port = IntentPort::new(&fixture.port, Some(FILLER));
        port.rerank_fault = Some((1, fault));
        let result = on_stopped_clock(
            second_rerank_started(&port),
            Box::pin(async {
                let started = Instant::now();
                let result = search_with(
                    &fixture,
                    &port,
                    "scheduler job",
                    low_confidence(IntentExpansion::Hyde),
                    small_budget(),
                )
                .await
                .unwrap();
                assert!(
                    started.elapsed() <= Duration::from_millis(1500),
                    "{fault:?}"
                );
                result
            }),
        )
        .await;
        assert_eq!(port.rerank_calls.load(Ordering::SeqCst), 2, "{fault:?}");
        assert_eq!(result.ranked, first.ranked, "{fault:?}");
        assert_eq!(result.routes["rerank"], RouteStatus::Ok, "{fault:?}");
        assert_eq!(
            result.routes["intent_expansion"],
            RouteStatus::Unavailable("intent_second_pass_unavailable".to_owned()),
            "{fault:?}"
        );
        for route in [Route::DenseIntent, Route::LexicalIntent] {
            assert_eq!(result.routes[route.name()], RouteStatus::Ok, "{fault:?}");
            assert!(!result.observations.route_ranks[&route].is_empty());
        }
    }
    clean(&fixture).await;
}

#[tokio::test]
async fn always_mode_at_a_small_budget_still_reranks_the_originals() {
    let fixture = published().await;
    let mut port = IntentPort::new(&fixture.port, Some(FILLER));
    port.delay = Duration::from_secs(5);
    let routes = StageEnd::watch_all(&[
        "retrieval.route.dense",
        "retrieval.route.lexical",
        "retrieval.route.identifier",
    ]);
    let result = on_stopped_clock(
        async {
            tokio::join!(chat_started(&port), routes.ended());
        },
        Box::pin(async {
            let started = Instant::now();
            let result = search_with(
                &fixture,
                &port,
                "scheduler job",
                SearchConfiguration {
                    intent_expansion: IntentExpansion::Hyde,
                    intent_deadline_ms: 5000,
                    ..SearchConfiguration::default()
                },
                small_budget(),
            )
            .await
            .unwrap();
            assert!(started.elapsed() <= Duration::from_millis(1500));
            result
        }),
    )
    .await;
    assert_eq!(
        result.routes["intent_expansion"],
        RouteStatus::Unavailable("intent_deadline_exceeded".to_owned())
    );
    assert_eq!(result.routes["rerank"], RouteStatus::Ok);
    assert!(result.ranked.iter().all(|ranked| ranked.score.is_some()));
    clean(&fixture).await;
}

#[tokio::test]
async fn a_grant_revoked_during_the_expansion_fails_the_search() {
    let fixture = published().await;
    // Keywords that find nothing with dense off, then a second pass.
    let empty = r#"{"passage":"Scheduler documentation","keywords":"!!!"}"#;
    for (reply, dense_enabled) in [(empty, false), (FILLER, true)] {
        let mut port = IntentPort::new(&fixture.port, Some(reply));
        port.revoke = Some(fixture.kernel.database.clone());
        let result = search_with(
            &fixture,
            &port,
            "scheduler job",
            SearchConfiguration {
                dense_enabled,
                ..low_confidence(IntentExpansion::Hyde)
            },
            RequestBudget::default(),
        )
        .await;
        assert_eq!(port.calls.load(Ordering::SeqCst), 1, "{reply}");
        assert!(
            matches!(result, Err(SearchError::PermissionsChanged)),
            "{reply}: {result:?}"
        );
        fixture
            .kernel
            .database
            .grant(
                "tester",
                &"workspace/default".parse().unwrap(),
                Right::Read,
                "test",
            )
            .unwrap();
    }
    clean(&fixture).await;
}

#[tokio::test]
async fn a_rejected_expansion_names_the_rule_it_broke() {
    let fixture = published().await;
    for (reply, reason) in [
        ("not json", "intent_guard_malformed"),
        (
            r#"{"passage":"Retry a job.","keywords":"retry"}"#,
            "intent_guard_protected_missing",
        ),
        (
            concat!(
                r#"{"passage":"Retry a job only after the third failure, "#,
                r#"within 5 minutes.","keywords":"retry"}"#,
            ),
            "intent_guard_added_number",
        ),
    ] {
        let port = IntentPort::new(&fixture.port, Some(reply));
        let result = run_configured(
            &fixture,
            &port,
            "retry a job only after the third failure",
            SearchConfiguration {
                intent_expansion: IntentExpansion::Hyde,
                rerank_enabled: false,
                ..SearchConfiguration::default()
            },
        )
        .await;
        assert_eq!(
            result.routes["intent_expansion"],
            RouteStatus::Unavailable(reason.to_owned()),
            "{reply}"
        );
    }
    clean(&fixture).await;
}
