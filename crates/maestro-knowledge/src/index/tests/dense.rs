//! The embedder's deadline: a port that never answers is dropped once it
//! passes, and the batch has no vectors.

use super::super::{Failure, dense::embed};
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardFields, Error, FakeModels, Limits, Message, ModelCard, ModelPort, Role, Room,
        RouterEntry,
    },
};
use std::{
    env, fs,
    future::{self, Future},
    num::{NonZeroU32, NonZeroUsize},
    process,
    sync::atomic::{AtomicUsize, Ordering},
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
        messages: &[Message],
    ) -> impl Future<Output = Result<String, Error>> + Send {
        FakeModels.chat(card, room, messages)
    }
}

/// An embedder's card of 4 dimensions, recorded in a store that is gone
/// once it is made.
fn card() -> ModelCard {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
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
    let root = env::temp_dir().join(format!(
        "maestro-knowledge-dense-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(&root).unwrap();
    card
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
