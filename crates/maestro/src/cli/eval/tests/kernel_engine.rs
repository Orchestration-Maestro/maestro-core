//! The engine on a real kernel, with the router and the search service
//! unreachable: what it names as a rung's provenance, the candidate reranker
//! its searches carry, and one failed outcome per call. Then how each search
//! and `ask` error is classed, out of time or failed.

use super::{
    super::{
        engine::{KernelEngine, expected_failure},
        manifest::AskSettings,
        rank_settings::SourcePriorSetting,
        rung_prompt::RungPrompt,
        runner::Engine as _,
        stages::{StageFailure, ask_failure, evidence_failure, search_failure},
    },
    support::{rung, suite},
};
use crate::{
    failure::Failure,
    knowledge::operations::{ask::tests::register_card, tests::Scratch},
};
use maestro_kernel::artifact::Digest;
use maestro_kernel::evidence::RequestBudget;
use maestro_kernel::{
    gateway::{Error as GatewayError, Role, RouterClient, Url},
    retrieval,
};
use maestro_knowledge::search::evidence::{CounterMode, ExpansionMode, ParentChainOrder};
use maestro_knowledge::{
    answer::{AskBudget, AskError, AskRequest, DEFAULT_MODEL, PromptVersion},
    eval::{AskOutcome, RunError, SearchOutcome},
    index::Qdrant,
    search::{SearchError, evidence::EvidenceError, routes::error::RouteError},
};
use std::{fs, io};

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

/// Full-section, UTF-8 ask settings with k 8, 9,000 evidence bytes, 900
/// output tokens and prompt v1.
fn v1_settings() -> AskSettings {
    AskSettings {
        expansion: ExpansionMode::FullSection,
        evidence_counter: CounterMode::Utf8,
        k: Some(8),
        evidence_bytes: Some(9000),
        output_tokens: Some(900),
        prompt: RungPrompt::Version(PromptVersion::V1),
        card: None,
    }
}

#[test]
fn a_search_and_an_ask_carry_the_rungs_configuration_budget_and_prompt() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();
    let settings = v1_settings();

    let (request, prompt) = engine.ask_call("question", &settings, None).unwrap();

    assert_eq!(
        request,
        AskRequest {
            collection: "collection".to_owned(),
            question: "question".to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            version: None,
            budget: AskBudget {
                k: 8,
                evidence_bytes: 9000,
                output_tokens: Some(900),
                ..AskBudget::default()
            },
        }
    );
    assert_eq!(prompt, PromptVersion::V1.into());
    let (default, default_prompt) = engine
        .ask_call("question", &AskSettings::default(), None)
        .unwrap();
    assert_eq!(default.budget, AskBudget::default());
    assert_eq!(default_prompt, PromptVersion::V2.into());
}

#[test]
fn a_search_carries_the_asks_budget_and_evidence_settings() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();
    let settings = v1_settings();

    let mut candidate = rung("r0");
    candidate.ask = None;
    let search = engine.search_request(&candidate, "question");
    assert_eq!(search.configuration, candidate.configuration.search());
    assert_eq!((search.collection, search.text), ("collection", "question"));
    assert_eq!(
        search.budget.evidence_bytes,
        RequestBudget::DEFAULT_SEARCH_EVIDENCE_BYTES
    );
    candidate.ask = None;
    candidate.search_budget = Some(RequestBudget {
        k: 4,
        evidence_bytes: 9_000,
        ..RequestBudget::default()
    });
    assert_eq!(
        engine
            .search_request(&candidate, "question")
            .budget
            .evidence_bytes,
        9_000
    );
    candidate.search_budget = None;
    candidate.ask = Some(settings.clone());
    let asking = engine.search_request(&candidate, "question").budget;
    assert_eq!((asking.k, asking.evidence_bytes), (8, 9000));
    candidate.ask = None;
    let unasked = engine.search_request(&candidate, "question").budget;
    assert_eq!(
        unasked.evidence_bytes,
        RequestBudget::DEFAULT_SEARCH_EVIDENCE_BYTES
    );
    let search = engine.search_request(&candidate, "question");
    assert_eq!(search.configuration, candidate.configuration.search());
    assert_eq!((search.collection, search.text), ("collection", "question"));
    assert_eq!(
        search.budget.evidence_bytes,
        RequestBudget::DEFAULT_SEARCH_EVIDENCE_BYTES
    );
    candidate.ask = Some(AskSettings {
        expansion: ExpansionMode::RelevantBlocks,
        evidence_counter: CounterMode::Utf8AnswerBound,
        ..settings
    });
    let search = engine.search_request(&candidate, "question");
    assert_eq!(search.evidence.expansion, ExpansionMode::RelevantBlocks);
    assert_eq!(
        search.evidence.evidence_counter,
        CounterMode::Utf8AnswerBound
    );
    assert_eq!((search.budget.k, search.budget.evidence_bytes), (8, 9000));
    candidate.ask = Some(AskSettings::default());
    let search = engine.search_request(&candidate, "question");
    let asked = AskBudget::default();
    assert_eq!(
        (
            search.budget.k,
            search.budget.evidence_bytes,
            search.budget.deadline_ms
        ),
        (asked.k, asked.evidence_bytes, asked.search_deadline_ms)
    );
}

#[test]
fn only_a_failed_document_lookup_fails_the_start_of_a_rung() {
    let lookup = RunError::Documents {
        source_ref: "a.md".to_owned(),
        error: io::Error::other("disk"),
    };
    let absent = RunError::<io::Error>::NoDocument {
        question: "q1".to_owned(),
        source_ref: "a.md".to_owned(),
    };

    assert!(matches!(expected_failure(&lookup), Failure::Failed(_)));
    assert!(matches!(expected_failure(&absent), Failure::Refused(_)));
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
    assert_eq!(engine.ask(&lexical, "question").outcome, AskOutcome::Failed);

    engine.start(&lexical, &suite(0, 1)).unwrap();
    let searched = engine.search(&lexical, "question").outcome;
    let asked = engine.ask(&lexical, "question").outcome;

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

#[test]
fn a_rung_with_an_active_source_prior_names_the_bound_table_and_searches_with_it() {
    let table = r#"{"schema": "maestro-source-classes/1", "rules": []}"#;
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let path = kernel.config_dir.join("source-classes.json");
    fs::write(&path, table).unwrap();
    fs::write(
        kernel.config_dir.join("bindings.toml"),
        format!("source_classes = '{}'\n", path.display()),
    )
    .unwrap();
    let (_, reranker) = register_card(&kernel, "collection", Role::Reranker, "rerank", b"r");
    let mut official_first = rung("r0");
    official_first.configuration.routes.dense = false;
    official_first.configuration.rerank.as_mut().unwrap().card =
        reranker.digest().as_str().to_owned();
    let mut off = official_first.clone();
    off.configuration.source_prior = Some(SourcePriorSetting::Off);
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let mut engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();

    let (start, _) = engine.start(&official_first, &suite(0, 1)).unwrap();
    assert_eq!(
        start.source_classes.as_deref(),
        Some(Digest::of(table.as_bytes()).as_str())
    );
    assert!(engine.search_context().unwrap().source_classes.is_some());
    assert_eq!(engine.provenance(&off).unwrap().source_classes, None);
}

#[test]
fn search_only_request_carries_parent_chain_expansion_and_order() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();
    let mut rung = rung("parent");
    rung.ask = None;
    rung.configuration.evidence_expansion = Some(ExpansionMode::ParentChain);
    for order in [
        ParentChainOrder::MinimumCompleteFirst,
        ParentChainOrder::LargestFittingParent,
    ] {
        rung.configuration.parent_chain_order = Some(order);
        let request = engine.search_request(&rung, "question");
        assert_eq!(request.evidence.expansion, ExpansionMode::ParentChain);
        assert_eq!(request.evidence.parent_chain_order, Some(order));
    }
}
