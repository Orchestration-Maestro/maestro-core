//! A search fails when a stage its rung enables did not run: out of time for
//! `deadline_exceeded`, failed otherwise. A stage the rung disables, and a
//! rerank with nothing to rerank, never fail it.

use super::super::stages::{StageFailure, stage_failure};
use maestro_kernel::evidence::RouteStatus;
use maestro_knowledge::search::{
    DEADLINE_EXCEEDED, DISABLED_BY_CONFIGURATION, NO_FUSED_CANDIDATES, SearchConfiguration,
};
use std::collections::BTreeMap;

/// Routes where every stage ran, but `stage`, which ended `status`.
fn routes(stage: &str, status: Option<&str>) -> BTreeMap<String, RouteStatus> {
    let mut routes: BTreeMap<String, RouteStatus> = ["dense", "lexical", "identifier", "rerank"]
        .into_iter()
        .map(|name| (name.to_owned(), RouteStatus::Ok))
        .collect();
    match status {
        Some(reason) => {
            routes.insert(
                stage.to_owned(),
                RouteStatus::Unavailable(reason.to_owned()),
            );
        }
        None => {
            routes.remove(stage);
        }
    }
    routes
}

#[test]
fn an_enabled_stage_that_did_not_run_fails_the_search() {
    let all = SearchConfiguration::default();
    let cases = [
        (
            "dense",
            Some(DEADLINE_EXCEEDED),
            Some(StageFailure::TimedOut),
        ),
        (
            "dense",
            Some("no embedder card"),
            Some(StageFailure::Failed),
        ),
        (
            "lexical",
            Some(DEADLINE_EXCEEDED),
            Some(StageFailure::TimedOut),
        ),
        (
            "lexical",
            Some("qdrant_unavailable"),
            Some(StageFailure::Failed),
        ),
        (
            "rerank",
            Some("router_unavailable"),
            Some(StageFailure::Failed),
        ),
        (
            "rerank",
            Some(DEADLINE_EXCEEDED),
            Some(StageFailure::TimedOut),
        ),
        ("rerank", None, Some(StageFailure::Failed)),
        ("rerank", Some(NO_FUSED_CANDIDATES), None),
        ("identifier", Some("identifier too common"), None),
        ("structured", Some("unsupported inventory"), None),
    ];
    for (stage, status, expected) in cases {
        assert_eq!(
            stage_failure(&all, &routes(stage, status)),
            expected,
            "{stage}: {status:?}"
        );
    }
    assert_eq!(stage_failure(&all, &routes("identifier", Some("x"))), None);
}

#[test]
fn a_stage_the_rung_disables_never_fails_the_search() {
    let none = SearchConfiguration {
        dense_enabled: false,
        lexical_enabled: false,
        rerank_enabled: false,
        routes_limit: 100,
        identifier_limit: 20,
        fusion_pool: 120,
        ..SearchConfiguration::default()
    };
    for stage in ["dense", "lexical", "rerank"] {
        assert_eq!(
            stage_failure(&none, &routes(stage, Some(DISABLED_BY_CONFIGURATION))),
            None
        );
        assert_eq!(
            stage_failure(&none, &routes(stage, Some(DEADLINE_EXCEEDED))),
            None
        );
    }
}

#[test]
fn the_first_enabled_stage_that_did_not_run_decides() {
    let all = SearchConfiguration::default();
    let mut both = routes("dense", Some("no embedder card"));
    both.insert(
        "rerank".to_owned(),
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
    );
    assert_eq!(stage_failure(&all, &both), Some(StageFailure::Failed));
    let without_dense = SearchConfiguration {
        dense_enabled: false,
        ..all
    };
    assert_eq!(
        stage_failure(&without_dense, &both),
        Some(StageFailure::TimedOut)
    );
}
