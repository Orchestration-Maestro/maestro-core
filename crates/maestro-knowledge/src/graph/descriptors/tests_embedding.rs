//! Compatible outputs are shared across arms; mismatches cannot inherit readiness.

use super::{DescriptorEmbedder, build, tests::fixture};
use crate::index::embedding_profile;
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
    future::Future,
    num::{NonZeroU32, NonZeroUsize},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

/// Count external embedding calls while retaining the real fake-model vector behavior.
#[derive(Debug, Default)]
struct CountingModels {
    /// Calls admitted to the model boundary.
    calls: AtomicUsize,
}

impl ModelPort for CountingModels {
    fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, Error>> + Send {
        self.calls.fetch_add(1, Ordering::Relaxed);
        FakeModels.embed(card, room, inputs)
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

/// Pinned synthetic embedding card with no real model I/O.
pub(super) fn card() -> ModelCard {
    let root = scratch_directory().unwrap();
    let card = ModelCard::record(
        &Store::new(&root),
        &CardFields {
            role: Role::Embedder,
            router_entry: RouterEntry::parse("embed").unwrap(),
            file_digest: Digest::of(b"synthetic embedding weights"),
            template_digest: None,
            server_build: "synthetic/1".into(),
            dimensions: NonZeroUsize::new(4),
            limits: Limits {
                context_tokens: NonZeroU32::new(512).unwrap(),
                output_tokens: None,
            },
            suite_results: vec![],
        },
    )
    .unwrap();
    fs::remove_dir_all(root).unwrap();
    card
}

#[tokio::test]
async fn compatible_arms_reuse_checked_vectors_without_an_embedding_call() {
    let (input, contexts) = fixture();
    let documents = build(&input, &contexts).unwrap();
    let card = card();
    let models = CountingModels::default();
    let embedder = DescriptorEmbedder {
        models: &models,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    let output = embedder.prepare(&input.pin, &documents, None).await;
    assert!(output.is_ok(), "{output:?}");
    let output = output.unwrap();
    assert_eq!(output.receipt().count, 3);
    let claim_index = output
        .documents
        .iter()
        .position(|document| document.kind == "claim")
        .unwrap();
    assert_eq!(
        output.vectors[claim_index],
        [
            -271.0 / 32768.0,
            -28725.0 / 32768.0,
            -24701.0 / 32768.0,
            -6385.0 / 32768.0
        ]
    );
    let reuse_only = DescriptorEmbedder {
        deadline: Duration::ZERO,
        ..embedder
    };
    let reused = reuse_only
        .prepare(&input.pin, &documents, Some(&output))
        .await
        .unwrap();
    assert_eq!(reused.vectors, output.vectors);
    assert_eq!(reused.receipt(), output.receipt());
    assert_eq!(models.calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn changed_builder_embedding_preprocessing_linking_content_or_pin_refuses_reuse() {
    let (input, contexts) = fixture();
    let documents = build(&input, &contexts).unwrap();
    let card = card();
    let embedder = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    let output = embedder
        .prepare(&input.pin, &documents, None)
        .await
        .unwrap();
    for change in 0..14 {
        let mut cached = output.clone();
        match change {
            0 => cached.receipt.profile.builder = Digest::of(b"different builder"),
            1 => cached.receipt.profile.embedding = Digest::of(b"different model"),
            2 => cached.receipt.profile.preprocessing = Digest::of(b"different preprocessing"),
            3 => cached.receipt.profile.linking = Digest::of(b"different linking"),
            4 => cached.receipt.pin.generation_id += 1,
            5 => cached.receipt.content = Digest::of(b"different source"),
            6 => cached.documents[0].text.push_str("invented prose"),
            7 => cached.vectors[0].clear(),
            8 => cached.vectors[0][0] = f32::NAN,
            9 => cached.vectors[0].fill(0.0),
            10 => {
                cached.vectors.pop();
            }
            11 => cached.receipt.pin.version = Some("2.0".into()),
            12 => cached.receipt.pin.collection_id = "other".into(),
            _ => cached.receipt.profile.dimensions += 1,
        }
        assert!(
            embedder
                .prepare(&input.pin, &documents, Some(&cached))
                .await
                .is_err(),
            "change {change} reused incompatible output"
        );
    }
}

#[tokio::test]
async fn descriptor_composition_is_model_free_but_mixed_embedding_pins_are_refused() {
    let (input, contexts) = fixture();
    let mut documents = build(&input, &contexts).unwrap();
    assert_eq!(documents.len(), 3);
    documents[0].pin.generation_id += 1;
    let card = card();
    let embedder = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    assert!(
        embedder
            .prepare(&input.pin, &documents, None)
            .await
            .is_err()
    );
    assert!(embedder.prepare(&input.pin, &[], None).await.is_err());
}

#[tokio::test]
async fn non_embedding_card_cannot_reuse_descriptor_readiness() {
    let (input, contexts) = fixture();
    let documents = build(&input, &contexts).unwrap();
    let valid_card = card();
    let mut cached = DescriptorEmbedder {
        models: &FakeModels,
        card: &valid_card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    }
    .prepare(&input.pin, &documents, None)
    .await
    .unwrap();
    let mut fields = valid_card.fields().clone();
    fields.role = Role::Reranker;
    fields.dimensions = None;
    let root = scratch_directory().unwrap();
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(root).unwrap();
    let models = CountingModels::default();
    let embedder = DescriptorEmbedder {
        models: &models,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    assert!(
        embedder
            .prepare(&input.pin, &documents, None)
            .await
            .is_err()
    );
    cached.receipt.profile.embedding = card.digest().clone();
    cached.receipt.profile.preprocessing = Digest::of(embedding_profile(&card).as_bytes());
    assert!(
        embedder
            .prepare(&input.pin, &documents, Some(&cached))
            .await
            .is_err()
    );
    assert_eq!(models.calls.load(Ordering::Relaxed), 0);
}
