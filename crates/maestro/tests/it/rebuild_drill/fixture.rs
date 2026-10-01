//! Frozen public questions, profiles and actual T029c search capture.

use super::{
    qdrant::point_ids,
    ranking_oracle::{self, Frozen, QuestionInput, QuestionRanking, Snapshot},
};
use crate::{knowledge_publish::StubRouter, support::Home};
use maestro_kernel::{
    artifact::{Digest, Store},
    chunk_set::Chunk,
    document::{Document, Revision},
    gateway::{ModelCard, RouterClient, Url},
    generation::GenerationState,
    retrieval::IDENTIFIER_PROFILE,
    scope::{LOCAL, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    index::Qdrant,
    search::{Reranker, SearchConfiguration, routes::dense::Embedder},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};
use tokio::runtime::Builder;
use ulid::Ulid;

#[derive(Clone, Copy)]
pub(super) struct FrozenInputs<'a> {
    pub(super) home: &'a Home,
    pub(super) qdrant_url: &'a str,
    pub(super) chunk_set: &'a str,
    pub(super) card: &'a str,
    pub(super) reranker: &'a str,
    pub(super) collection_digest: &'a Digest,
    pub(super) corpus_digest: &'a Digest,
    pub(super) suite_digest: &'a Digest,
}

pub(super) struct SearchEnvironment<'a> {
    pub(super) home: &'a Home,
    pub(super) router: &'a StubRouter,
    pub(super) qdrant_url: &'a str,
}

struct CaptureRecords {
    database: Arc<Database>,
    chunks: HashMap<String, Chunk>,
    revisions: HashMap<String, Revision>,
    documents: HashMap<String, Document>,
}

pub(super) fn suite_questions(bytes: &[u8]) -> Vec<QuestionInput> {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .map(|line| {
            let question: Value = serde_json::from_str(line).unwrap();
            QuestionInput {
                id: question["id"].as_str().unwrap().to_owned(),
                language: question["language"].as_str().unwrap().to_owned(),
                answerable: question["answerable"].as_bool().unwrap(),
                question: question["question"].as_str().unwrap().to_owned(),
                expected: question["expected"].clone(),
            }
        })
        .collect()
}

pub(super) fn question_inputs(questions: &[QuestionRanking]) -> Vec<QuestionInput> {
    questions
        .iter()
        .map(|question| QuestionInput {
            id: question.id.clone(),
            language: question.language.clone(),
            answerable: question.answerable,
            question: question.question.clone(),
            expected: question.expected.clone(),
        })
        .collect()
}

pub(super) fn frozen_template(input: FrozenInputs<'_>) -> Frozen {
    let FrozenInputs {
        home,
        qdrant_url,
        chunk_set,
        card,
        reranker,
        collection_digest,
        corpus_digest,
        suite_digest,
    } = input;
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let set = database.chunk_set(&scopes, chunk_set).unwrap().unwrap();
    let generation = database
        .published_generation(&scopes, "synthetic")
        .unwrap()
        .unwrap();
    let profiles = BTreeMap::from([
        ("chunk".to_owned(), set.chunk_profile),
        ("counter".to_owned(), set.counter_contract_id),
        ("embedding".to_owned(), generation.embedding_profile),
        ("sparse".to_owned(), generation.sparse_profile),
        ("identifier".to_owned(), IDENTIFIER_PROFILE.to_owned()),
        ("embedder_card".to_owned(), card.to_owned()),
        ("reranker_card".to_owned(), reranker.to_owned()),
    ]);
    Frozen {
        suite_digest: suite_digest.as_str().to_owned(),
        collection_digest: collection_digest.as_str().to_owned(),
        corpus_manifest_digest: corpus_digest.as_str().to_owned(),
        collection: "synthetic".to_owned(),
        chunk_set: chunk_set.to_owned(),
        card_digest: card.to_owned(),
        reranker_digest: reranker.to_owned(),
        profiles,
        grants: grant_names(&scopes),
        backend: BTreeMap::from([
            (
                "image".to_owned(),
                concat!(
                    "qdrant/qdrant@sha256:",
                    "12364fe851b9f17356fc88189fc06d1b521262e04659ec7345975b00c9246a10"
                )
                .to_owned(),
            ),
            ("version".to_owned(), "1.19.1".to_owned()),
            ("rest_port".to_owned(), "16533".to_owned()),
            ("grpc_port".to_owned(), "16534".to_owned()),
            ("telemetry".to_owned(), "disabled".to_owned()),
            ("settings".to_owned(), "maestro-index-defaults/1".to_owned()),
            ("endpoint".to_owned(), qdrant_url.to_owned()),
        ]),
        tie_policy: concat!(
            "rrf-k=60;route-rank=one-based;score-desc;chunk-id-asc;",
            "reranker-score-desc;stable-ties"
        )
        .to_owned(),
        model_identity: "FakeModels synthetic protocol simulation/1".to_owned(),
        qualification_digest: Digest::of(native_parity().as_bytes()).as_str().to_owned(),
        rerank_depth: u32::try_from(SearchConfiguration::default().rerank_depth.get())
            .expect("the default depth fits u32"),
        point_ids: Vec::new(),
    }
}

pub(super) fn capture_snapshot(
    environment: &SearchEnvironment<'_>,
    template: &Frozen,
    questions: &[QuestionInput],
    progress_job: Ulid,
) -> (Snapshot, BTreeSet<String>, Value) {
    let (home, router, qdrant_url) = (environment.home, environment.router, environment.qdrant_url);
    let database = Arc::new(home.database());
    let scopes = database.visible(LOCAL).unwrap();
    let generation = database
        .published_generation(&scopes, &template.collection)
        .unwrap()
        .unwrap();
    assert_eq!(generation.state, GenerationState::Published);
    let set = database
        .chunk_set(&scopes, &generation.chunk_set_id)
        .unwrap()
        .unwrap();
    let revisions = database.revisions(&scopes, &template.collection).unwrap();
    let chunks = database.chunks(&scopes, &set.id).unwrap();
    assert_eq!(revisions.len(), 28);
    assert!(!chunks.is_empty());
    let revision_records: HashMap<_, _> = revisions
        .into_iter()
        .map(|revision| (revision.id.clone(), revision))
        .collect();
    let chunk_records: HashMap<_, _> = chunks
        .into_iter()
        .map(|chunk| (chunk.id.clone(), chunk))
        .collect();
    let document_records = document_records(database.as_ref(), &scopes, &revision_records);
    let qdrant = Qdrant::new(qdrant_url).unwrap();
    let port = RouterClient::new(Url::parse(router.url()).unwrap()).unwrap();
    let store = Store::new(home.data().join("artifacts"));
    let embed_card =
        ModelCard::load(&store, &Digest::parse(&template.card_digest).unwrap()).unwrap();
    let rerank_card =
        ModelCard::load(&store, &Digest::parse(&template.reranker_digest).unwrap()).unwrap();
    let embedder = Embedder {
        port: &port,
        card: &embed_card,
    };
    let reranker = Reranker {
        port: &port,
        card: &rerank_card,
    };
    let point_ids = point_ids(qdrant_url, &generation, &chunk_records, &revision_records);
    assert_eq!(point_ids.len(), chunk_records.len());
    let records = CaptureRecords {
        database: Arc::clone(&database),
        chunks: chunk_records,
        revisions: revision_records,
        documents: document_records,
    };
    let capture = ranking_oracle::SearchCapture {
        database: Arc::clone(&records.database),
        principal: LOCAL,
        qdrant: &qdrant,
        embedder: &embedder,
        reranker: &reranker,
        collection: &template.collection,
        chunks: &records.chunks,
        revisions: &records.revisions,
        documents: &records.documents,
    };
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let mut captured = Vec::with_capacity(questions.len());
    for question in questions {
        captured.push(
            runtime
                .block_on(ranking_oracle::capture_question(&capture, question))
                .unwrap(),
        );
    }
    let mut frozen = template.clone();
    frozen.chunk_set = set.id;
    frozen.grants = grant_names(&scopes);
    frozen.point_ids = point_ids.iter().cloned().collect();
    frozen
        .profiles
        .insert("chunk".to_owned(), set.chunk_profile);
    frozen
        .profiles
        .insert("counter".to_owned(), set.counter_contract_id);
    frozen
        .profiles
        .insert("embedding".to_owned(), generation.embedding_profile);
    frozen
        .profiles
        .insert("sparse".to_owned(), generation.sparse_profile);
    let initial_progress = database
        .last_progress(&scopes, progress_job)
        .unwrap()
        .unwrap()
        .data;
    (
        Snapshot {
            schema: "maestro-synthetic-rankings/1".to_owned(),
            frozen,
            generation: generation.id,
            questions: captured,
        },
        point_ids,
        initial_progress,
    )
}

pub(super) fn capture_one(
    environment: &SearchEnvironment<'_>,
    baseline: &Snapshot,
    question: &QuestionRanking,
) -> QuestionRanking {
    let input = QuestionInput {
        id: question.id.clone(),
        language: question.language.clone(),
        answerable: question.answerable,
        question: question.question.clone(),
        expected: question.expected.clone(),
    };
    let database = Arc::new(environment.home.database());
    let scopes = database.visible(LOCAL).unwrap();
    let set = database
        .chunk_set(&scopes, &baseline.frozen.chunk_set)
        .unwrap()
        .unwrap();
    let revisions: HashMap<_, _> = database
        .revisions(&scopes, &baseline.frozen.collection)
        .unwrap()
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let chunks: HashMap<_, _> = database
        .chunks(&scopes, &set.id)
        .unwrap()
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let documents = document_records(database.as_ref(), &scopes, &revisions);
    let records = CaptureRecords {
        database,
        chunks,
        revisions,
        documents,
    };
    capture_with_records(environment, baseline, &input, &records)
}

fn capture_with_records(
    environment: &SearchEnvironment<'_>,
    baseline: &Snapshot,
    question: &QuestionInput,
    records: &CaptureRecords,
) -> QuestionRanking {
    let qdrant = Qdrant::new(environment.qdrant_url).unwrap();
    let port = RouterClient::new(Url::parse(environment.router.url()).unwrap()).unwrap();
    let store = Store::new(environment.home.data().join("artifacts"));
    let embed_card = ModelCard::load(
        &store,
        &Digest::parse(&baseline.frozen.card_digest).unwrap(),
    )
    .unwrap();
    let rerank_card = ModelCard::load(
        &store,
        &Digest::parse(&baseline.frozen.reranker_digest).unwrap(),
    )
    .unwrap();
    let embedder = Embedder {
        port: &port,
        card: &embed_card,
    };
    let reranker = Reranker {
        port: &port,
        card: &rerank_card,
    };
    let capture = ranking_oracle::SearchCapture {
        database: Arc::clone(&records.database),
        principal: LOCAL,
        qdrant: &qdrant,
        embedder: &embedder,
        reranker: &reranker,
        collection: &baseline.frozen.collection,
        chunks: &records.chunks,
        revisions: &records.revisions,
        documents: &records.documents,
    };
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(ranking_oracle::capture_question(&capture, question))
        .unwrap()
}

fn document_records(
    database: &Database,
    scopes: &ScopeSet,
    revisions: &HashMap<String, Revision>,
) -> HashMap<String, Document> {
    revisions
        .values()
        .map(|revision| {
            let document = database
                .document(scopes, &revision.document_id)
                .unwrap()
                .unwrap();
            (document.id.clone(), document)
        })
        .collect()
}

pub(super) fn grant_names(scopes: &ScopeSet) -> Vec<String> {
    scopes.granted().map(ToString::to_string).collect()
}

fn native_parity() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-knowledge/src/prepare/native-parity.json"
    ))
}
