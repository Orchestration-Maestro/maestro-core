//! The engine on a real kernel, with the router and the search service
//! unreachable: what it names as a rung's provenance, the candidate reranker
//! its searches carry, and one failed outcome per call. Then how each search
//! and `ask` error is classed, out of time or failed.

use super::{
    super::{
        engine::{KernelEngine, ask_failure, evidence_failure, search_failure},
        runner::Engine as _,
        stages::StageFailure,
    },
    support::{rung, suite},
};
use crate::knowledge::operations::{ask::tests::register_card, tests::Scratch};
use maestro_kernel::{
    gateway::{Error as GatewayError, Role, RouterClient, Url},
    retrieval,
};
use maestro_knowledge::{
    answer::AskError,
    eval::{AskOutcome, SearchOutcome},
    index::Qdrant,
    search::{SearchError, evidence::EvidenceError, routes::error::RouteError},
};

/// An address where nothing listens.
const NOWHERE: &str = "http://127.0.0.1:1";

#[test]
fn the_engine_names_the_generation_chunk_set_and_every_card_and_sees_drift() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, reranker) = register_card(&kernel, "collection", Role::Reranker, "rerank", b"r");
    let (_, answerer) = register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");
    let generation = kernel
        .database
        .published_generation(&kernel.scopes, "collection")
        .unwrap()
        .unwrap();
    let mut candidate = rung("r0");
    candidate.configuration.routes.dense = false;
    candidate.configuration.rerank.as_mut().unwrap().card = reranker.digest().as_str().to_owned();
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let mut engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();

    assert!(engine.search_context().is_none());
    let (start, expected) = engine.start(&candidate, &suite(0, 2)).unwrap();
    assert_eq!(expected, [Vec::new(), Vec::new()]);
    assert_eq!(start.generation, generation.id);
    assert_eq!(start.chunk_set, "chunk-set");
    assert_eq!(start.embedder, None);
    assert_eq!(start.reranker.as_deref(), Some(reranker.digest().as_str()));
    assert_eq!(start.answerer.as_deref(), Some(answerer.digest().as_str()));
    let context = engine.search_context().unwrap();
    assert_eq!(
        context.reranker.map(|reranker| reranker.card),
        Some(&reranker)
    );
    assert!(context.embedder.is_none());
    assert_eq!(engine.provenance(&candidate).unwrap(), start);

    let (_, later) = register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"b");
    let end = engine.provenance(&candidate).unwrap();
    assert_eq!(end.answerer.as_deref(), Some(later.digest().as_str()));
    assert_ne!(end, start);
}

#[test]
fn with_the_services_down_each_search_and_ask_fails_once() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");
    let mut lexical = rung("r0");
    lexical.configuration.routes.dense = false;
    lexical.configuration.rerank = None;
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let mut engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();
    assert_eq!(
        engine.search(&lexical, "question").outcome,
        SearchOutcome::Failed
    );
    assert_eq!(engine.ask(&lexical, "question"), AskOutcome::Failed);

    engine.start(&lexical, &suite(0, 1)).unwrap();
    let searched = engine.search(&lexical, "question").outcome;
    let asked = engine.ask(&lexical, "question");

    assert!(
        !matches!(searched, SearchOutcome::Ranked(_)),
        "{searched:?}"
    );
    assert!(!matches!(asked, AskOutcome::Answered { .. }), "{asked:?}");
}

#[test]
fn only_a_deadline_classes_a_search_as_out_of_time() {
    let cases = [
        (SearchError::AdmissionTimedOut, StageFailure::TimedOut),
        (SearchError::PermissionCheckTimedOut, StageFailure::TimedOut),
        (
            SearchError::InvalidRequest {
                reason: "k".to_owned(),
            },
            StageFailure::Failed,
        ),
        (
            SearchError::Admission(RouteError::UnknownGeneration {
                collection: "c".to_owned(),
            }),
            StageFailure::Failed,
        ),
        (
            SearchError::Kernel(retrieval::Error::UnknownOrInaccessible),
            StageFailure::Failed,
        ),
        (
            SearchError::EvidenceLoad {
                reason: "r".to_owned(),
            },
            StageFailure::Failed,
        ),
        (SearchError::PermissionsChanged, StageFailure::Failed),
        (SearchError::WorkerFailed, StageFailure::Failed),
    ];
    for (error, expected) in cases {
        assert_eq!(search_failure(&error), expected, "{error:?}");
    }
    assert_eq!(
        evidence_failure(&EvidenceError::TimedOut),
        StageFailure::TimedOut
    );
    assert_eq!(
        evidence_failure(&EvidenceError::NotVisible),
        StageFailure::Failed
    );
}

#[test]
fn an_ask_is_out_of_time_when_its_search_evidence_or_answerer_is() {
    let json = serde_json::from_str::<u8>("x").unwrap_err();
    let cases = [
        (AskError::TimedOut, StageFailure::TimedOut),
        (
            AskError::Search(SearchError::AdmissionTimedOut),
            StageFailure::TimedOut,
        ),
        (
            AskError::Search(SearchError::WorkerFailed),
            StageFailure::Failed,
        ),
        (
            AskError::Evidence(EvidenceError::TimedOut),
            StageFailure::TimedOut,
        ),
        (
            AskError::Evidence(EvidenceError::NotVisible),
            StageFailure::Failed,
        ),
        (AskError::InvalidRequest("model"), StageFailure::Failed),
        (
            AskError::Backend(GatewayError::Unavailable {
                reason: "down".to_owned(),
            }),
            StageFailure::Failed,
        ),
        (AskError::EvidenceIntegrity, StageFailure::Failed),
        (AskError::Json(json), StageFailure::Failed),
    ];
    for (error, expected) in cases {
        assert_eq!(ask_failure(&error), expected, "{error:?}");
    }
}
