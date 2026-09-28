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
        Query, Reranker, SearchError,
        deadline::DEADLINE_EXCEEDED,
        orchestrate::{rerank_candidates, route_outcome, search_outcome},
        route_execution::{dense_outcome, lexical_outcome, structured_outcome},
        routes::{dense::Embedder, error::RouteError, identifier::search_identifiers},
    },
};
use maestro_kernel::{
    evidence::RouteStatus, gateway::Role, retrieval::InventoryRequest, telemetry::stage::Outcome,
};
use std::num::NonZeroUsize;
use tokio::time::Instant;

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

    let dense = dense_outcome(&query, Some(&embedder), passed).await;
    let lexical = lexical_outcome(&query, passed).await;
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
        NonZeroUsize::MIN,
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
