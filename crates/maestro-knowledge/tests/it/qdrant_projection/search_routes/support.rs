//! Qdrant points and pinned generations shared by route tests.

use super::{backends::Backend, kernel::Kernel};
use maestro_kernel::{
    gateway::ModelCard,
    generation::{Generation, GenerationState},
    retrieval::IDENTIFIER_PROFILE,
    scope::{Right, ScopeSet},
};
use qdrant_client::{
    Payload, Qdrant as Client,
    qdrant::{
        CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, Distance, FieldType, Modifier,
        NamedVectors, PointStruct, SparseVectorParamsBuilder, SparseVectorsConfigBuilder,
        UpsertPointsBuilder, Vector, VectorParamsBuilder, VectorsConfigBuilder,
    },
};
use serde_json::json;

pub(super) const TEXT: &str = "Maestro private retrieval phrase";

/// A generation whose collection and profiles match this test's model card.
pub(super) fn generation(
    kernel: &Kernel,
    card: &ModelCard,
    id: i64,
    state: GenerationState,
    sparse_profile: &str,
) -> Generation {
    Generation {
        id,
        collection_id: kernel.collection.clone(),
        chunk_set_id: kernel.chunk_set.clone(),
        embedding_profile: format!("dense/1:sha256:{}", card.digest().as_str()),
        sparse_profile: sparse_profile.to_owned(),
        state,
        point_count: Some(2),
        published_at: Some("2026-09-26T00:00:00Z".to_owned()),
    }
}

/// A scope limited to this kernel's collection, rather than its whole workspace.
pub(super) fn collection_scopes(kernel: &Kernel) -> ScopeSet {
    let name = format!("reader-{}", kernel.collection);
    let scope = kernel.collection_scope().parse().unwrap();
    kernel
        .database
        .grant(&name, &scope, Right::Read, "test")
        .unwrap();
    kernel.database.visible(&name).unwrap()
}

/// The collection name of `generation`, not its moving alias.
pub(super) fn collection(generation: &Generation) -> String {
    format!("maestro-{}-g{}", generation.collection_id, generation.id)
}

/// The Qdrant client used to seed a backend.
pub(super) fn client(backend: &Backend) -> Client {
    Client::from_url(&backend.url)
        .skip_compatibility_check()
        .build()
        .unwrap()
}

/// Creates a dense and sparse test collection and its scope keyword index.
pub(in super::super) async fn create_collection(
    backend: &Backend,
    generation: &Generation,
    card: &ModelCard,
) {
    let name = collection(generation);
    let dimensions = card
        .fields()
        .dimensions
        .expect("test collection card has dimensions")
        .get();
    let dimensions = u64::try_from(dimensions).expect("Qdrant dimension fits u64");
    let mut dense = VectorsConfigBuilder::default();
    dense.add_named_vector_params(
        "dense",
        VectorParamsBuilder::new(dimensions, Distance::Cosine),
    );
    let mut sparse = SparseVectorsConfigBuilder::default();
    sparse.add_named_vector_params(
        "bm25",
        SparseVectorParamsBuilder::default().modifier(Modifier::Idf),
    );
    let client = client(backend);
    client
        .create_collection(
            CreateCollectionBuilder::new(&name)
                .vectors_config(dense)
                .sparse_vectors_config(sparse),
        )
        .await
        .unwrap();
    client
        .create_field_index(
            CreateFieldIndexCollectionBuilder::new(&name, "scope_tags", FieldType::Keyword)
                .wait(true),
        )
        .await
        .unwrap();
}

/// A point with a single provenance scope.
#[expect(
    clippy::too_many_arguments,
    reason = "the test point maps independent payload and vector fields to Qdrant"
)]
pub(super) fn point(
    id: usize,
    chunk: &str,
    revision: &str,
    scope: &str,
    dense: &[f32],
    sparse_indices: &[u32],
    sparse_values: &[f32],
) -> PointStruct {
    let vectors = NamedVectors::default()
        .add_vector("dense", dense.to_vec())
        .add_vector("bm25", Vector::new_sparse(sparse_indices, sparse_values));
    let payload = Payload::try_from(json!({
        "chunk_id": chunk,
        "revision_id": revision,
        "scope_tags": [scope],
    }))
    .unwrap();
    PointStruct::new(
        format!("00000000-0000-4000-8000-{id:012x}"),
        vectors,
        payload,
    )
}

/// A payload-only exact-identifier match for filtered-scroll tests.
pub(super) fn identifier_point(id: &str, chunk: &str, revision: &str, scope: &str) -> PointStruct {
    let vectors = NamedVectors::default()
        .add_vector("dense", vec![1.0, 0.0, 0.0])
        .add_vector("bm25", Vector::new_sparse([], []));
    let payload = Payload::try_from(json!({
        "chunk_id": chunk,
        "revision_id": revision,
        "scope_tags": [scope],
        "version": "9.0.22",
        "identifier_profile": IDENTIFIER_PROFILE,
        "identifiers": ["ERR-042"],
    }))
    .unwrap();
    PointStruct::new(id, vectors, payload)
}

/// Writes `points` to the generation's own collection.
pub(super) async fn upsert(backend: &Backend, generation: &Generation, points: Vec<PointStruct>) {
    client(backend)
        .upsert_points(UpsertPointsBuilder::new(collection(generation), points).wait(true))
        .await
        .unwrap();
}

/// Removes the test collection when the backend is real; fake instances are per test.
pub(super) async fn cleanup(backend: &Backend, generations: &[&Generation]) {
    if backend.name == "qdrant" {
        let client = client(backend);
        for generation in generations {
            client
                .delete_collection(collection(generation))
                .await
                .unwrap();
        }
    }
}
