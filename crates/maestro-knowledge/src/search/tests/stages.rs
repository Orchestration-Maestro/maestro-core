//! The outcome each end of a search, a route or the rerank gives its stage,
//! and the reason code every route and the rerank give a passed deadline.

use super::{
    rerank::{FakePort, candidate, card},
    support::CandidateDb,
};
use crate::{
    index::Qdrant,
    query::understand,
    search::{
        DISABLED_BY_CONFIGURATION, Query, Reranker, Route, SearchError, SearchObservations,
        deadline::{DEADLINE_EXCEEDED, Deadlines, from_budget},
        orchestrate::search_outcome,
        rerank::rerank_candidates,
        route_execution::route_outcome,
        route_execution::{dense_outcome, lexical_outcome, structured_outcome},
        routes::{
            dense::Embedder,
            error::RouteError,
            identifier::{search_identifiers, search_identifiers_enabled},
        },
    },
};
use maestro_kernel::{
    artifact::Digest,
    evidence::{Budget, Bundle, Passage, RequestBudget, RouteStatus, Schema, Span, Trace},
    gateway::Role,
    retrieval::InventoryRequest,
    telemetry::stage::Outcome,
};
use std::{collections::BTreeMap, num::NonZeroUsize};
use tokio::time::{Duration, Instant};

#[tokio::test]
async fn disabled_routes_short_circuit_and_enabled_failures_remain_unavailable() {
    let fixture = CandidateDb::new(b"prepared text", "docs");
    let qdrant = Qdrant::new("http://127.0.0.1:1").unwrap();
    let query = Query {
        generation: &fixture.generation,
        scopes: &fixture.scopes,
        text: "ERR-042",
        limit: 10,
        version: None,
        qdrant: &qdrant,
    };
    let port = FakePort::scores(Vec::new());
    let embedder_card = card(Role::Embedder, 128);
    let embedder = Embedder {
        port: &port,
        card: &embedder_card,
    };
    let cutoffs = from_budget(Instant::now(), RequestBudget::default());
    let disabled_dense = dense_outcome(false, &query, Some(&embedder), &cutoffs).await;
    assert!(port.calls.lock().unwrap().is_empty());
    let disabled_lexical = lexical_outcome(false, &query, Instant::now()).await;
    let disabled_identifier = search_identifiers_enabled(
        false,
        &query,
        fixture.database.clone(),
        &understand("ERR-042"),
        Instant::now(),
    )
    .await;
    for outcome in [disabled_dense, disabled_lexical, disabled_identifier] {
        assert_eq!(
            outcome.status,
            RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned())
        );
    }

    let failed = lexical_outcome(true, &query, Instant::now()).await;
    assert_eq!(
        failed.status,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
}

#[test]
fn observations_keep_route_ranks_and_accept_assembled_passage_order() {
    let mut observations = SearchObservations {
        route_ranks: BTreeMap::from([(Route::Dense, vec!["d1".to_owned(), "d2".to_owned()])]),
        reranked_chunk_ids: vec!["d2".to_owned(), "d1".to_owned()],
        ..SearchObservations::default()
    };
    let bundle = Bundle {
        schema: Schema::V1,
        collection: "docs".to_owned(),
        generation: 1,
        query: "query".to_owned(),
        lang: "en".to_owned(),
        routes: BTreeMap::new(),
        passages: vec![passage(1), passage(2)],
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_tokens: 0,
            limit: 1,
            counter: None,
            estimated: false,
        },
        request_budget: None,
        inventory: None,
        trace: vec![trace(1, &["chunk-z", "chunk-a"]), trace(2, &["chunk-b"])],
    };
    observations.observe_assembly(&bundle);
    assert_eq!(observations.route_ranks[&Route::Dense], ["d1", "d2"]);
    assert_eq!(observations.reranked_chunk_ids, ["d2", "d1"]);
    assert_eq!(
        observations.assembled_passages,
        [
            vec!["chunk-z".to_owned(), "chunk-a".to_owned()],
            vec!["chunk-b".to_owned()]
        ]
    );
}

fn passage(n: u32) -> Passage {
    Passage {
        n,
        section_id: None,
        document_id: "doc".to_owned(),
        revision_id: "revision".to_owned(),
        title: "title".to_owned(),
        section_path: Vec::new(),
        version: None,
        source_ref: "source".to_owned(),
        span: Span { start: 0, end: 0 },
        digest: Digest::of(b""),
        text: String::new(),
        windowed: false,
        alternates: Vec::new(),
    }
}

fn trace(n: u32, chunk_ids: &[&str]) -> Trace {
    Trace {
        n,
        score: None,
        routes: Vec::new(),
        chunk_ids: chunk_ids
            .iter()
            .map(|chunk_id| (*chunk_id).to_owned())
            .collect(),
        procedural: false,
    }
}

#[test]
fn a_route_that_ran_is_ok_and_one_that_ran_out_of_time_is_a_timeout() {
    let statuses = [
        (RouteStatus::Ok, Outcome::Ok),
        (
            RouteStatus::Unavailable("deadline_exceeded".to_owned()),
            Outcome::Timeout,
        ),
        (
            RouteStatus::Unavailable("payload: deadline_exceeded".to_owned()),
            Outcome::Unavailable,
        ),
        (
            RouteStatus::Unavailable("route deadline elapsed".to_owned()),
            Outcome::Unavailable,
        ),
        (
            RouteStatus::Unavailable("Qdrant search failed".to_owned()),
            Outcome::Unavailable,
        ),
    ];
    for (status, outcome) in statuses {
        assert_eq!(route_outcome(&status), outcome, "{status:?}");
    }
}

#[test]
fn a_search_refused_by_its_contract_or_rights_is_refused_not_failed() {
    let errors = [
        (
            SearchError::InvalidRequest {
                reason: "k".to_owned(),
            },
            Outcome::Refused,
        ),
        (
            SearchError::Admission(RouteError::UnknownGeneration {
                collection: "absent".to_owned(),
            }),
            Outcome::Refused,
        ),
        (SearchError::PermissionsChanged, Outcome::Refused),
        (SearchError::AdmissionTimedOut, Outcome::Timeout),
        (SearchError::PermissionCheckTimedOut, Outcome::Timeout),
        (
            SearchError::EvidenceLoad {
                reason: "corrupt".to_owned(),
            },
            Outcome::Error,
        ),
        (SearchError::WorkerFailed, Outcome::Error),
    ];
    for (error, outcome) in errors {
        assert_eq!(search_outcome(&error), outcome, "{error}");
    }
}

#[tokio::test]
async fn each_route_and_the_rerank_report_a_passed_deadline_as_its_code() {
    let fixture = CandidateDb::new(b"prepared text", "docs");
    let qdrant = Qdrant::new("http://127.0.0.1:1").unwrap();
    let query = Query {
        generation: &fixture.generation,
        scopes: &fixture.scopes,
        text: "ERR-042",
        limit: 10,
        version: None,
        qdrant: &qdrant,
    };
    let port = FakePort::scores(vec![1.0]);
    let embedder_card = card(Role::Embedder, 128);
    let reranker_card = card(Role::Reranker, 128);
    let embedder = Embedder {
        port: &port,
        card: &embedder_card,
    };
    let passed = Instant::now();
    let inventory = InventoryRequest::DocumentsBySet { set: None };
    let understood = understand("ERR-042");

    let cutoffs = Deadlines {
        expires: passed,
        routes: passed,
        setup: passed,
        work: passed,
        window: Duration::ZERO,
    };
    let dense = dense_outcome(true, &query, Some(&embedder), &cutoffs).await;
    let lexical = lexical_outcome(true, &query, passed).await;
    let identifier =
        search_identifiers(&query, fixture.database.clone(), &understood, passed).await;
    let structured = structured_outcome(
        &query,
        fixture.database.clone(),
        Ok(Some(&inventory)),
        passed,
    )
    .await;
    let (_, rerank) = rerank_candidates(
        &understood,
        vec![candidate("candidate", 1.0, "prepared text")],
        Some(&Reranker {
            port: &port,
            card: &reranker_card,
        }),
        Some(NonZeroUsize::MIN),
        passed,
    )
    .await;

    let timed_out = RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned());
    let statuses = [
        ("dense", dense.status),
        ("lexical", lexical.status),
        ("identifier", identifier.status),
        ("structured", structured.route.status),
        ("rerank", rerank),
    ];
    for (route, status) in statuses {
        assert_eq!(status, timed_out, "{route}");
        assert_eq!(route_outcome(&status), Outcome::Timeout, "{route}");
    }
}
