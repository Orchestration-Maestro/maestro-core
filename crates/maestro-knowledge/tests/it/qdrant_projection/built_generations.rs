//! A generation builds in its own collection, a dense vector of the card's
//! dimensions compared by cosine and a sparse vector weighted by IDF, then
//! takes the collection's alias and records its profiles; each point carries
//! its chunk's payload and the vectors of its prepared input; and the same
//! chunk set gives the same point IDs.

use super::{
    backends::{Backend, backends},
    kernel::{Kernel, VERSION},
    models::{Embedder, embedder},
    support::{alias_of, collection_of, point_id, publish},
};
use maestro_kernel::{
    gateway::{FakeModels, ModelPort as _, Room},
    generation::GenerationState,
};
use maestro_knowledge::{
    index::Report,
    lexical::{AverageLength, Passage},
};
use qdrant_client::{
    Payload,
    qdrant::{Distance, Modifier, vector_output::Vector, vectors_config::Config},
};
use serde_json::{Value, json};
use std::slice;

#[tokio::test]
async fn a_generation_builds_in_its_own_collection_then_takes_the_alias() {
    for backend in backends("a_generation_builds_in_its_own_collection_then_takes_the_alias") {
        builds_in_its_own_collection(&backend).await;
    }
}

/// 30 guides give 91 chunks: two batches, of 64 and 27.
async fn builds_in_its_own_collection(backend: &Backend) {
    let kernel = Kernel::with_guides(30);
    let (card, port) = (embedder(8), Embedder::default());
    let report = publish(&kernel, backend, &port, &card).await.unwrap();
    let collection = collection_of(&kernel, 1);
    let profile = format!("dense/1:sha256:{}", card.digest().as_str());
    let expected = Report {
        collection: kernel.collection.clone(),
        chunk_set: kernel.chunk_set.clone(),
        generation: 1,
        qdrant_collection: collection.clone(),
        alias: alias_of(&kernel),
        points: 91,
        embedding_profile: profile.clone(),
        sparse_profile: "bm25-en-fr/1".to_owned(),
        retired: None,
    };
    assert_eq!(report, expected, "{}", backend.name);
    holds_its_points_behind_the_alias(backend, &kernel, &collection).await;
    records_its_profiles(&kernel, &profile);
    let batches: Vec<usize> = port.calls().iter().map(Vec::len).collect();
    assert_eq!(batches, [64, 27]);
    assert_eq!(port.rooms(), [Room::Free, Room::Free]);
}

/// `collection`, the first generation of `kernel`, has a dense vector of 8
/// dimensions compared by cosine and a sparse vector weighted by IDF, holds
/// its 91 points, and takes the alias.
async fn holds_its_points_behind_the_alias(backend: &Backend, kernel: &Kernel, collection: &str) {
    let parameters = backend.parameters(collection).await;
    let vectors = parameters.vectors_config.and_then(|config| config.config);
    let Some(Config::ParamsMap(named)) = vectors else {
        panic!("{} {collection}: no named vectors", backend.name);
    };
    let dense = &named.map["dense"];
    assert_eq!(
        (dense.size, dense.distance),
        (8, i32::from(Distance::Cosine))
    );
    let sparse = &parameters.sparse_vectors_config.unwrap().map["bm25"];
    assert_eq!(sparse.modifier, Some(i32::from(Modifier::Idf)));
    assert_eq!(backend.count(collection).await, 91);
    let alias = backend.alias(&alias_of(kernel)).await;
    assert_eq!(alias.as_deref(), Some(collection));
}

/// The first generation of `kernel` is published, with its 91 points, the
/// dense profile `profile` and the analyzer's sparse profile.
fn records_its_profiles(kernel: &Kernel, profile: &str) {
    let recorded = kernel
        .database
        .generation(&kernel.scopes, 1)
        .unwrap()
        .unwrap();
    assert_eq!(recorded.state, GenerationState::Published);
    assert_eq!(recorded.point_count, Some(91));
    assert_eq!(recorded.embedding_profile, profile);
    assert_eq!(recorded.sparse_profile, "bm25-en-fr/1");
    let published = kernel
        .database
        .published_generation(&kernel.scopes, &kernel.collection);
    assert_eq!(published.unwrap(), Some(recorded));
}

#[tokio::test]
async fn each_point_carries_its_chunks_payload_and_vectors() {
    for backend in backends("each_point_carries_its_chunks_payload_and_vectors") {
        carries_its_payload_and_vectors(&backend).await;
    }
}

/// The lead chunk of the first guide has no section; the first guide occurs
/// in two sources and carries a version, the second in one and none.
async fn carries_its_payload_and_vectors(backend: &Backend) {
    let kernel = Kernel::with_guides(3);
    let card = embedder(8);
    let report = publish(&kernel, backend, &Embedder::default(), &card)
        .await
        .unwrap();
    let chunks = kernel.chunks();
    let inputs: Vec<String> = chunks.iter().map(|chunk| kernel.input(chunk)).collect();
    let terms: usize = inputs
        .iter()
        .map(|input| Passage::new(input).term_count())
        .sum();
    let terms = f64::from(u32::try_from(terms).unwrap());
    let passages = f64::from(u32::try_from(inputs.len()).unwrap());
    let average = AverageLength::new(terms / passages).unwrap();
    let scope = kernel.collection_scope();
    let docs = format!("{scope}/source/docs");
    let mirror = format!("{scope}/source/mirror");
    let cases = [
        (
            "chunk-0-lead",
            json!([]),
            json!(["workspace/default", scope, docs, mirror]),
            json!(VERSION),
        ),
        (
            "chunk-0-1",
            json!(["Guide 0", "Retries"]),
            json!(["workspace/default", scope, docs, mirror]),
            json!(VERSION),
        ),
        (
            "chunk-1-2",
            json!(["Guide 1", "Logs"]),
            json!(["workspace/default", scope, docs]),
            Value::Null,
        ),
    ];
    for (id, section_path, scope_tags, version) in cases {
        let (chunk, input) = chunks
            .iter()
            .zip(&inputs)
            .find(|(chunk, _)| chunk.id == id)
            .unwrap();
        let point = backend.point(&report.qdrant_collection, point_id(id)).await;
        let expected = json!({
            "chunk_id": id,
            "revision_id": chunk.revision_id,
            "section_path": section_path,
            "scope_tags": scope_tags,
            "version": version,
            "source_kind": "guide",
        });
        let payload = Value::from(Payload::from(point.payload));
        assert_eq!(payload, expected, "{} {id}", backend.name);
        let vectors = point.vectors.unwrap();
        let Some(Vector::Sparse(found)) = vectors.get_vector_by_name("bm25") else {
            panic!("{} {id}: no sparse vector", backend.name);
        };
        let sparse = Passage::new(input).vector(average);
        assert_eq!(found.indices, sparse.indices(), "{id}");
        assert_eq!(found.values.len(), sparse.values().len());
        for (found, expected) in found.values.iter().zip(sparse.values()) {
            assert!((found - expected).abs() < 1e-6, "{id}");
        }
        let fake = FakeModels
            .embed(&card, Room::Free, slice::from_ref(input))
            .await
            .unwrap();
        let norm = fake[0]
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        let Some(Vector::Dense(dense)) = vectors.get_vector_by_name("dense") else {
            panic!("{} {id}: no dense vector", backend.name);
        };
        assert_eq!(dense.data.len(), 8);
        for (found, value) in dense.data.iter().zip(&fake[0]) {
            assert!((found - value / norm).abs() < 1e-5, "{id}");
        }
    }
}

#[tokio::test]
async fn the_same_chunk_set_gives_the_same_point_ids() {
    for backend in backends("the_same_chunk_set_gives_the_same_point_ids") {
        gives_the_same_point_ids(&backend).await;
    }
}

/// Two kernels whose chunk sets hold the same chunks, in collections of
/// their own; then the first published again.
async fn gives_the_same_point_ids(backend: &Backend) {
    let (first, second) = (Kernel::with_guides(3), Kernel::with_guides(3));
    let (card, port) = (embedder(8), Embedder::default());
    let report = publish(&first, backend, &port, &card).await.unwrap();
    let other = publish(&second, backend, &port, &card).await.unwrap();
    let ids = backend.ids(&report.qdrant_collection).await;
    assert_eq!(ids.len(), 10);
    assert_eq!(backend.ids(&other.qdrant_collection).await, ids);
    for chunk in ["chunk-0-1", "chunk-0-lead", "chunk-1-2"] {
        assert!(ids.contains(point_id(chunk)), "{} {chunk}", backend.name);
    }
    let calls = port.calls().len();
    let again = publish(&first, backend, &port, &card).await.unwrap();
    assert_eq!(again, report);
    assert_eq!(
        port.calls().len(),
        calls,
        "a published generation embeds nothing"
    );
    assert_eq!(backend.ids(&report.qdrant_collection).await, ids);
}
