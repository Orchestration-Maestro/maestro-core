//! Rank-stage coverage for intent rerank headers and their missing count.

use super::super::rerank::{FakePort, card};
use super::{Corpus, bounded, evidence_input, texts};
use crate::search::{
    IntentExpansion, RerankHeader, SearchConfiguration, candidate_enrichment,
    evidence::{EvidenceCounter, assemble_evidence},
    rank_stage,
    rerank::Reranker,
};
use maestro_kernel::{evidence::RouteStatus, gateway::Role, retrieval::ReadControl};
use std::{
    num::NonZeroUsize,
    sync::Arc,
    sync::atomic::AtomicBool,
    time::{Duration as StdDuration, Instant as StdInstant},
};

fn intent_headers() -> SearchConfiguration {
    SearchConfiguration {
        rerank_header: RerankHeader::HeadingPath,
        rerank_depth: NonZeroUsize::new(1).unwrap(),
        intent_expansion: IntentExpansion::Hyde,
        intent_rerank_additions: 2,
        ..SearchConfiguration::default()
    }
}

#[tokio::test]
async fn rerank_header_covers_intent_additions_without_changing_candidate_text() {
    let corpus = Corpus::new();
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let configuration = intent_headers();
    let admitted = corpus.admitted("run a task", configuration);
    let mut pool = corpus.pool();
    pool.rerank_extra = 2;
    let reranker_card = card(Role::Reranker, 8192);
    let ranking = rank_stage::rank(
        corpus.database.clone(),
        Some(&Reranker {
            port: &port,
            card: &reranker_card,
        }),
        &admitted,
        "run a task",
        pool,
    )
    .await
    .unwrap();
    assert_eq!(ranking.status, RouteStatus::Ok);
    assert_eq!(texts(&ranking), corpus.prepared());
    assert_eq!(ranking.header_missing, Some(0));
    let sent = &port.calls.lock().unwrap()[0].documents;
    assert_eq!(sent.len(), 3);
    for (index, heading) in ["Release notes", "Running", "Running"].iter().enumerate() {
        assert_eq!(
            sent[index],
            format!("Notes > {heading}\n\n{}", corpus.prepared()[index])
        );
    }
}

#[tokio::test]
async fn a_missing_header_is_counted_across_the_ranked_intent_additions() {
    let corpus = Corpus::with_one_missing_header();
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let admitted = corpus.admitted("run a task", intent_headers());
    let mut pool = corpus.pool();
    pool.rerank_extra = 2;
    let reranker_card = card(Role::Reranker, 8192);
    let ranking = rank_stage::rank(
        corpus.database.clone(),
        Some(&Reranker {
            port: &port,
            card: &reranker_card,
        }),
        &admitted,
        "run a task",
        pool,
    )
    .await
    .unwrap();

    assert_eq!(ranking.status, RouteStatus::Ok);
    assert_eq!(texts(&ranking), corpus.prepared());
    assert_eq!(ranking.header_missing, Some(1));
    let sent = &port.calls.lock().unwrap()[0].documents;
    assert_eq!(sent[0], corpus.prepared()[0]);
    assert!(sent[1].starts_with("Notes > Running\n\n"));
    assert!(sent[2].starts_with("Notes > Running\n\n"));
}

#[test]
fn an_unavailable_intent_addition_header_does_not_remove_other_headers() {
    let corpus = Corpus::new();
    let mut chunks = corpus.chunks.clone();
    chunks[2].revision_id = "missing-revision".to_owned();
    let mut candidates = chunks.iter().zip(corpus.prepared()).collect::<Vec<_>>();
    let configuration = intent_headers();
    let control = ReadControl {
        deadline: StdInstant::now() + StdDuration::from_secs(10),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let enriched = candidate_enrichment::enrich(
        (&corpus.database, &corpus.scopes, &control),
        &candidate_enrichment::Settings {
            configuration,
            query: "run a task",
            generation: &corpus.generation,
            deadline: StdInstant::now() + StdDuration::from_secs(10),
        },
        &mut candidates,
    );
    assert!(enriched.headers.contains_key(&chunks[0].id));
    assert!(enriched.headers.contains_key(&chunks[1].id));
    assert!(!enriched.headers.contains_key(&chunks[2].id));
}

#[tokio::test]
async fn rerank_header_preserves_evidence_and_works_with_enriched_text() {
    let corpus = Corpus::new();
    let mut bundles = Vec::new();
    let mut candidate_texts = None;
    for header in [RerankHeader::Off, RerankHeader::HeadingPath] {
        let port = FakePort::scores(vec![0.5, 0.5, 0.5]);
        let admitted = corpus.admitted(
            "run a task",
            SearchConfiguration {
                rerank_header: header,
                ..bounded()
            },
        );
        let ranking = corpus.rank(&port, &admitted, "run a task").await;
        let sent = port.calls.lock().unwrap()[0].documents.clone();
        let candidate_text = texts(&ranking)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if header == RerankHeader::Off {
            candidate_texts = Some(candidate_text.clone());
        } else {
            assert_eq!(candidate_texts.as_ref(), Some(&candidate_text));
            let includes_expanded_section = candidate_text
                .iter()
                .any(|text| text.contains("Start here.\n\n1. Select a task.\n2. Press Run."));
            assert!(includes_expanded_section);
        }
        for (index, item) in ranking.ranked.iter().enumerate() {
            assert_eq!(
                sent[index],
                format!(
                    "{}{}",
                    item.candidate.header.as_deref().unwrap_or_default(),
                    item.candidate.text
                )
            );
            assert_eq!(item.candidate.header.is_none(), header == RerankHeader::Off);
        }
        let bundle = assemble_evidence(
            corpus.database.clone(),
            evidence_input(&corpus, admitted, ranking.ranked),
            EvidenceCounter::Utf8Bytes,
        )
        .await
        .unwrap();
        bundles.push(serde_json::to_vec(&bundle).unwrap());
    }
    assert_eq!(bundles[0], bundles[1]);
}
