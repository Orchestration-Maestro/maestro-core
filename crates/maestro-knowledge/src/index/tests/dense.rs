//! The embedder's deadline: a port that never answers is dropped once it
//! passes, and the batch has no vectors.

use super::super::{Failure, dense::embed};
use crate::prepare::tests::support::identity;
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardFields, ChatRequest, Error, FakeModels, Limits, ModelCard, ModelPort, Role, Room,
        RouterEntry,
    },
};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    future::{self, Future},
    num::{NonZeroU32, NonZeroUsize},
    slice,
    sync::{Arc, Mutex},
    time::Duration,
};

/// A port whose embedder never answers; its other models are the fake's.
#[derive(Debug)]
struct Silent;

impl ModelPort for Silent {
    fn embed(
        &self,
        _card: &ModelCard,
        _room: Room,
        _inputs: &[String],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, Error>> + Send {
        future::pending()
    }

    fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        FakeModels.rerank(card, room, query, documents)
    }

    fn tokenize(
        &self,
        card: &ModelCard,
        room: Room,
        text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        FakeModels.tokenize(card, room, text)
    }

    fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        FakeModels.chat(card, room, request)
    }
}

/// An embedder's card of 4 dimensions, recorded in a store that is gone
/// once it is made.
fn card() -> ModelCard {
    let fields = CardFields {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").unwrap(),
        file_digest: Digest::of(b"model file"),
        template_digest: None,
        server_build: "b1".to_owned(),
        dimensions: NonZeroUsize::new(4),
        limits: Limits {
            context_tokens: NonZeroU32::new(512).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let root = scratch_directory().unwrap();
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(&root).unwrap();
    card
}

#[derive(Debug, Clone, Default)]
struct RecordingEmbedder(Arc<Mutex<Vec<Vec<String>>>>);

impl ModelPort for RecordingEmbedder {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        self.0.lock().unwrap().push(inputs.to_vec());
        FakeModels.embed(card, room, inputs).await
    }

    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        FakeModels.rerank(card, room, query, documents).await
    }

    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        FakeModels.tokenize(card, room, text).await
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> Result<String, Error> {
        FakeModels.chat(card, room, request).await
    }
}

#[tokio::test]
async fn indexing_formats_v2_documents_once_before_embedding() {
    let root = scratch_directory().unwrap();
    let card =
        ModelCard::record_v2(&Store::new(&root), &identity(Digest::of(b"qualification"))).unwrap();
    fs::remove_dir_all(root).unwrap();
    let port = RecordingEmbedder::default();
    let inputs = ["raw passage".to_owned()];
    embed(&port, &card, &inputs, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(port.0.lock().unwrap().as_slice(), [["doc: raw passage"]]);
}

#[tokio::test]
async fn a_document_starting_with_its_card_prefix_is_formatted_as_raw_text() {
    let root = scratch_directory().unwrap();
    let card =
        ModelCard::record_v2(&Store::new(&root), &identity(Digest::of(b"qualification"))).unwrap();
    fs::remove_dir_all(root).unwrap();
    let port = RecordingEmbedder::default();
    let inputs = ["doc: text that is part of the document".to_owned()];

    embed(&port, &card, &inputs, Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(
        port.0.lock().unwrap().as_slice(),
        [["doc: doc: text that is part of the document"]]
    );
}

#[tokio::test]
async fn single_and_batch_v2_embedding_agree_with_once_formatted_inputs() {
    let root = scratch_directory().unwrap();
    let card =
        ModelCard::record_v2(&Store::new(&root), &identity(Digest::of(b"qualification"))).unwrap();
    fs::remove_dir_all(root).unwrap();
    let port = RecordingEmbedder::default();
    let inputs = ["first passage".to_owned(), "second passage".to_owned()];
    let batch = embed(&port, &card, &inputs, Duration::from_secs(5))
        .await
        .unwrap();
    let mut singles = Vec::new();
    let batch_inputs = port.0.lock().unwrap().clone();
    for input in &inputs {
        singles.extend(
            embed(&port, &card, slice::from_ref(input), Duration::from_secs(5))
                .await
                .unwrap(),
        );
    }
    let calls = port.0.lock().unwrap().clone();

    assert_eq!(
        batch_inputs,
        [["doc: first passage", "doc: second passage"]]
    );
    assert_eq!(
        calls,
        [
            vec![
                "doc: first passage".to_owned(),
                "doc: second passage".to_owned()
            ],
            vec!["doc: first passage".to_owned()],
            vec!["doc: second passage".to_owned()],
        ]
    );
    assert_eq!(batch, singles);
}

#[tokio::test]
async fn an_embedder_that_never_answers_is_dropped_at_its_deadline() {
    let deadline = Duration::from_millis(20);
    let inputs = ["a passage".to_owned()];
    let failure = embed(&Silent, &card(), &inputs, deadline)
        .await
        .unwrap_err();
    assert!(
        matches!(failure, Failure::TimedOut(after) if after == deadline),
        "{failure}"
    );
}

#[tokio::test]
async fn an_embedder_that_answers_in_time_gives_one_checked_vector_per_input() {
    let inputs = ["a passage".to_owned(), "another".to_owned()];
    let card = card();
    let vectors = embed(&FakeModels, &card, &inputs, Duration::from_secs(5))
        .await
        .unwrap();
    let expected = FakeModels.embed(&card, Room::Free, &inputs).await.unwrap();
    assert_eq!(vectors, expected);
}
