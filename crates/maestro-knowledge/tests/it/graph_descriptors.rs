//! The public descriptor adapter deletes and recreates its actual gRPC collection.
#![cfg(test)]

use super::qdrant_projection;
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{CardFields, FakeModels, Limits, ModelCard, Role, RouterEntry},
};
use maestro_knowledge::{
    graph::descriptors::{
        Descriptor, DescriptorEmbedder, DescriptorProjection, DescriptorQdrant, DescriptorQuery,
    },
    index::{CollectionLayout, Qdrant, RetrievalProjectionPort},
};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    num::{NonZeroU32, NonZeroUsize},
    time::Duration,
};

/// Frozen synthetic model card; the public adapter never starts a model server.
fn card() -> ModelCard {
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
async fn graph_descriptors_qdrant_delete_and_recreate_preserves_payloads_and_other_collections() {
    let qdrant = Qdrant::new(&qdrant_projection::synthetic_fake_qdrant_url()).unwrap();
    qdrant
        .create_collection(
            "passage-survivor",
            CollectionLayout {
                dense_dimensions: 4,
                dense_present: true,
                dense_distance: "Cosine".into(),
                sparse_present: true,
                sparse_modifier: Some("Idf".into()),
            },
        )
        .await
        .unwrap();
    let projection = DescriptorQdrant::new(&qdrant);
    let documents: Vec<Descriptor> = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/synthetic/graph/descriptors.json"
    ))
    .unwrap();
    let card = card();
    let embedder = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    let output = embedder
        .prepare(&documents[0].pin, &documents, None)
        .await
        .unwrap();
    projection.rebuild(&output).await.unwrap();
    projection.verify(&output).await.unwrap();
    let query = DescriptorQuery {
        kind: "claim".into(),
        vector: vec![1.0; 4],
        limit: 1,
    };
    let before = projection
        .lookup(output.receipt(), query.clone())
        .await
        .unwrap();
    assert_eq!(before.len(), 1);
    projection.delete(output.receipt()).await.unwrap();
    assert!(projection.verify(&output).await.is_err());
    assert!(
        projection
            .lookup(output.receipt(), query.clone())
            .await
            .is_err()
    );
    assert!(qdrant.exists("passage-survivor").await.unwrap());
    let rebuilt = embedder
        .prepare(&documents[0].pin, &documents, None)
        .await
        .unwrap();
    projection.rebuild(&rebuilt).await.unwrap();
    let after = projection.lookup(rebuilt.receipt(), query).await.unwrap();
    assert_eq!(after[0].id, before[0].id);
    assert_eq!(after[0].payload, before[0].payload);
    assert_eq!(rebuilt.receipt(), output.receipt());
}
