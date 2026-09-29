//! The engine's pieces that need no search service: a rung's reranker taken
//! by digest without selection, the retrieval rank of a bundle's documents,
//! and what an answer gives the ladder.

use super::super::{
    candidates::candidate_reranker,
    documents::{bundle_documents, ranked_documents},
    engine::{answer_outcome, asked_reply_cap, rejected_checks},
    runner::RejectedCheck,
};
use crate::{
    failure::Failure,
    knowledge::operations::{ask::tests::register_card, tests::Scratch},
};
use maestro_kernel::{
    artifact::Digest,
    evidence::{Budget, Bundle, Passage, RouteStatus, Schema, Span, Trace},
    gateway::{Error as GatewayError, Role},
};
use maestro_knowledge::{
    answer::{
        Answer, AnswerCitation, AnswerModel, AnswerRefusal, AskError, RefusalCode,
        RegisteredAnswerer, Rejection,
    },
    eval::{AskOutcome, SectionRef},
    search::{DEADLINE_EXCEEDED, SearchConfiguration, SearchError, evidence::ChunkSetDocuments},
};
use std::collections::BTreeMap;

#[test]
fn a_registered_unselected_reranker_is_used_without_selecting_it() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, reranker) = register_card(&kernel, "collection", Role::Reranker, "rerank", b"r");

    let found = candidate_reranker(&kernel, "collection", Some(reranker.digest().clone()));

    assert_eq!(found.unwrap(), Some(reranker));
    let selected = kernel
        .database
        .selected_model_card(&kernel.scopes, "collection", Role::Reranker)
        .unwrap();
    assert!(selected.is_none());
    assert!(matches!(
        candidate_reranker(&kernel, "collection", None),
        Ok(None)
    ));
}

#[test]
fn a_reranker_card_of_another_role_or_unregistered_is_refused() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, answerer) = register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");

    assert!(matches!(
        candidate_reranker(&kernel, "collection", Some(answerer.digest().clone())),
        Err(Failure::Refused(reason)) if reason.contains("reranker role")
    ));
    assert!(matches!(
        candidate_reranker(&kernel, "collection", Some(Digest::of(b"unregistered"))),
        Err(Failure::Refused(reason)) if reason.contains("not registered")
    ));
    assert!(matches!(
        candidate_reranker(&kernel, "other", Some(answerer.digest().clone())),
        Err(Failure::Refused(reason)) if reason.contains("not registered")
    ));
}

/// A passage `n` of `document`.
fn passage(n: u32, document: &str) -> Passage {
    Passage {
        n,
        section_id: None,
        document_id: document.to_owned(),
        revision_id: "revision".to_owned(),
        title: String::new(),
        section_path: Vec::new(),
        version: None,
        source_ref: "source".to_owned(),
        span: Span { start: 0, end: 1 },
        digest: Digest::of(b"text"),
        text: "text".to_owned(),
        windowed: false,
        alternates: Vec::new(),
    }
}

/// The trace of passage `n` over `chunks`.
fn trace(n: u32, chunks: &[&str]) -> Trace {
    Trace {
        parent_context_of: Vec::new(),
        n,
        score: None,
        routes: Vec::new(),
        chunk_ids: chunks.iter().map(|chunk| (*chunk).to_owned()).collect(),
        procedural: false,
    }
}

#[test]
fn bundle_documents_rank_by_their_passages_best_chunk_not_by_reading_order() {
    let bundle = Bundle {
        schema: Schema::V1,
        collection: "collection".to_owned(),
        generation: 1,
        query: "question".to_owned(),
        lang: "en".to_owned(),
        routes: BTreeMap::new(),
        passages: vec![
            passage(1, "late"),
            passage(2, "first"),
            passage(3, "unranked"),
            passage(4, "late"),
        ],
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_tokens: 1,
            limit: 10,
            counter: None,
            estimated: true,
        },
        request_budget: None,
        inventory: None,
        trace: vec![
            trace(1, &["c3"]),
            trace(2, &["c9", "c0"]),
            trace(3, &["c-none"]),
            trace(4, &["c1"]),
        ],
    };
    let order = ["c0", "c1", "c2", "c3", "c9"].map(str::to_owned);

    assert_eq!(
        bundle_documents(&bundle, &order),
        ["first", "late", "late", "unranked"]
    );
}

/// An answer citing the section `section` of `source_ref`, or refusing.
fn answer(citations: &[(&str, &str)], refusal: Option<RefusalCode>) -> Answer {
    Answer {
        schema: "maestro-answer/1".to_owned(),
        collection: "collection".to_owned(),
        generation: 1,
        question: "question".to_owned(),
        lang: "en".to_owned(),
        answer: String::new(),
        citations: citations
            .iter()
            .enumerate()
            .map(|(index, (source_ref, section))| AnswerCitation {
                n: u32::try_from(index + 1).unwrap(),
                chunk_id: format!("chunk-{}", index + 1),
                section_id: Some((*section).to_owned()),
                source_ref: (*source_ref).to_owned(),
                title: String::new(),
                section_path: Vec::new(),
                span: [index + 1, index + 2],
            })
            .collect(),
        model: AnswerModel {
            router_entry: "qwen3-4b".to_owned(),
            card_id: None,
        },
        uncalibrated: true,
        refusal: refusal.map(|code| AnswerRefusal {
            code,
            message: String::new(),
        }),
        closest: Vec::new(),
        rejections: Vec::new(),
        routes: BTreeMap::new(),
        delivered: Vec::new(),
        reply_cap: None,
    }
}

/// A configuration whose searches check no stage.
fn unchecked() -> SearchConfiguration {
    SearchConfiguration {
        dense_enabled: false,
        lexical_enabled: false,
        rerank_enabled: false,
        ..SearchConfiguration::default()
    }
}

#[test]
fn an_answer_gives_its_citations_documents_or_its_refusal() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let documents = ChunkSetDocuments::read(&kernel.database, &kernel.scopes, "chunk-set").unwrap();

    assert_eq!(
        answer_outcome(
            &answer(&[("source:docs", "section"), ("elsewhere", "s")], None),
            &documents,
            1,
            &unchecked()
        ),
        AskOutcome::Answered {
            citations: vec![
                SectionRef {
                    document_id: "document".to_owned(),
                    revision_id: Some("revision".to_owned()),
                    chunk_id: Some("chunk-1".to_owned()),
                    section_id: Some("section".to_owned()),
                    span: Some([1, 2]),
                    component: None,
                },
                SectionRef {
                    document_id: String::new(),
                    revision_id: None,
                    chunk_id: Some("chunk-2".to_owned()),
                    section_id: Some("s".to_owned()),
                    span: Some([2, 3]),
                    component: None,
                },
            ],
            invented_literals: 0,
        }
    );
    assert_eq!(
        answer_outcome(
            &answer(&[], Some(RefusalCode::NoEvidence)),
            &documents,
            1,
            &unchecked()
        ),
        AskOutcome::Refused(RefusalCode::NoEvidence)
    );
}

#[test]
fn citations_from_an_answer_outside_the_pinned_generation_have_no_revision() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let documents = ChunkSetDocuments::read(&kernel.database, &kernel.scopes, "chunk-set").unwrap();
    let mut stale = answer(&[("source:docs", "section")], None);
    stale.generation = 2;

    assert!(matches!(
        answer_outcome(&stale, &documents, 1, &unchecked()),
        AskOutcome::Answered { citations, .. }
            if citations.iter().all(|citation| citation.revision_id.is_none())
    ));
}

#[test]
fn an_answer_from_a_search_that_did_not_run_its_rung_fails() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let documents = ChunkSetDocuments::read(&kernel.database, &kernel.scopes, "chunk-set").unwrap();
    let ran = BTreeMap::from(
        ["dense", "lexical", "rerank"].map(|stage| (stage.to_owned(), RouteStatus::Ok)),
    );
    let mut late_dense = answer(&[("source:docs", "section")], None);
    late_dense.routes = ran.clone();
    late_dense.routes.insert(
        "dense".to_owned(),
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
    );
    let mut unranked = answer(&[], Some(RefusalCode::NotFound));
    unranked.routes = ran.clone();
    unranked.routes.insert(
        "rerank".to_owned(),
        RouteStatus::Unavailable("router_unavailable".to_owned()),
    );
    let mut clean = answer(&[], Some(RefusalCode::NotFound));
    clean.routes = ran;
    let all = SearchConfiguration::default();

    assert_eq!(
        answer_outcome(&late_dense, &documents, 1, &all),
        AskOutcome::TimedOut
    );
    assert_eq!(
        answer_outcome(&unranked, &documents, 1, &all),
        AskOutcome::Failed
    );
    assert_eq!(
        answer_outcome(&clean, &documents, 1, &all),
        AskOutcome::Refused(RefusalCode::NotFound)
    );
    assert_eq!(
        answer_outcome(&unranked, &documents, 1, &unchecked()),
        AskOutcome::Refused(RefusalCode::NotFound)
    );
}

#[test]
fn the_ranked_list_holds_each_ranked_chunks_document_once_the_first_10() {
    let order: Vec<String> = ["c-unknown", "c0", "c1", "c0-again"]
        .into_iter()
        .map(str::to_owned)
        .chain((2..14).map(|index| format!("c{index}")))
        .collect();
    let document_of = |chunk: &str| -> Option<&'static str> {
        match chunk {
            "c-unknown" => None,
            "c0-again" => Some("d0"),
            chunk => Some(chunk.replacen('c', "d", 1).leak()),
        }
    };

    let ranked = ranked_documents(&order, document_of);
    let expected: Vec<String> = (0..10).map(|index| format!("d{index}")).collect();
    assert_eq!(ranked, expected);
    assert!(ranked_documents(&[], document_of).is_empty());

    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let documents = ChunkSetDocuments::read(&kernel.database, &kernel.scopes, "chunk-set").unwrap();
    let order = ["unknown", "chunk"].map(str::to_owned);
    assert_eq!(
        ranked_documents(&order, |chunk| documents.document_of_chunk(chunk)),
        ["document"]
    );
}

#[test]
fn an_answers_rejected_attempts_keep_their_checks_without_their_tokens() {
    let mut repaired = answer(&[("source:docs", "section")], None);
    repaired.rejections = vec![
        Rejection {
            attempt: 1,
            check: "unsupported_literal",
            tokens: vec!["secret".to_owned()],
        },
        Rejection {
            attempt: 2,
            check: "citation",
            tokens: Vec::new(),
        },
    ];

    assert_eq!(
        rejected_checks(&repaired),
        [
            RejectedCheck {
                attempt: 1,
                check: "unsupported_literal",
            },
            RejectedCheck {
                attempt: 2,
                check: "citation",
            },
        ]
    );
}

#[test]
fn an_answer_hands_the_reply_cap_its_chats_ran_with_to_the_ladder() {
    let answered = Answer {
        reply_cap: Some(2048),
        ..answer(&[], None)
    };

    assert_eq!(asked_reply_cap(&Ok(answered), None, None), Some(2048));
}

#[test]
fn a_chat_that_timed_out_or_failed_keeps_the_reply_cap_it_was_asked_with() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (id, card) = register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");
    let answerer = RegisteredAnswerer { id, card };
    let unavailable = GatewayError::Unavailable {
        reason: "the router is down".to_owned(),
    };

    for failed in [AskError::TimedOut, AskError::Backend(unavailable)] {
        assert_eq!(
            asked_reply_cap(&Err(failed), Some(&answerer), Some(900)),
            Some(900)
        );
    }
}

#[test]
fn an_ask_that_failed_before_its_chat_has_no_reply_cap() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (id, card) = register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");
    let answerer = RegisteredAnswerer { id, card };
    let searched = Err(AskError::Search(SearchError::AdmissionTimedOut));

    assert_eq!(asked_reply_cap(&searched, Some(&answerer), Some(900)), None);
}
