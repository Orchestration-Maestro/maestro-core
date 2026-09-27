//! A generation builds in its own collection, a dense vector of the card's
//! dimensions compared by cosine and a sparse vector weighted by IDF, then
//! takes the collection's alias and records its profiles; each point carries
//! its chunk's payload and the vectors of its prepared input; and the same
//! chunk set gives the same point IDs.

use super::{
    backends::{Backend, backends, fake},
    kernel::{Kernel, VERSION},
    models::{Embedder, embedder},
    search_routes::support::create_collection,
    support::{alias_of, collection_of, point_id, projection, publish},
};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::{FakeModels, ModelPort as _, Room},
    generation::{GenerationState, NewGeneration},
    retrieval::IDENTIFIER_PROFILE,
};
use maestro_knowledge::{
    index::{Projection, Report},
    lexical::{AverageLength, Passage},
    search::{DEFAULT_DEPTH, SearchContext, SearchRequest, search},
};
use qdrant_client::{
    Payload,
    qdrant::{Distance, Modifier, vector_output::Vector, vectors_config::Config},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, num::NonZeroUsize, ops::ControlFlow, slice};

#[tokio::test]
async fn a_markerless_published_generation_degrades_then_republishes_without_early_alias_move() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let card = embedder(8);
    let port = Embedder::default();
    let profile = format!("dense/1:sha256:{}", card.digest().as_str());
    let legacy = kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: kernel.collection.clone(),
            chunk_set_id: kernel.chunk_set.clone(),
            embedding_profile: profile,
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    kernel.database.verify_generation(legacy.id, 0).unwrap();
    kernel.database.publish_generation(legacy.id).unwrap();
    let legacy = kernel
        .database
        .generation(&kernel.scopes, legacy.id)
        .unwrap()
        .unwrap();
    let qdrant = backend.client();
    create_collection(&backend, &legacy).await;
    let alias = alias_of(&kernel);
    let old_collection = collection_of(&kernel, legacy.id);
    backend.point_alias(&alias, &old_collection).await;

    let context: SearchContext<'_, Embedder> = SearchContext {
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: None,
        reranker: None,
    };
    let identifier = SearchRequest {
        collection: &kernel.collection,
        text: "ERR-042",
        version: None,
        budget: RequestBudget::default(),
        rerank_depth: DEFAULT_DEPTH,
    };
    let identifier_result = Box::pin(search(&context, &identifier)).await.unwrap();
    assert_eq!(
        identifier_result.routes.get("identifier"),
        Some(&RouteStatus::Unavailable(
            "search projection missing; publish a new generation".to_owned()
        ))
    );
    assert!(identifier_result.ranked.is_empty());
    assert!(identifier_result.inventory.is_none());

    let inventory = SearchRequest {
        text: "how many documents",
        ..identifier
    };
    let inventory_result = Box::pin(search(&context, &inventory)).await.unwrap();
    assert_eq!(
        inventory_result.routes.get("structured"),
        Some(&RouteStatus::Unavailable(
            "search projection missing; publish a new generation".to_owned()
        ))
    );
    assert!(inventory_result.ranked.is_empty());
    assert!(inventory_result.inventory.is_none());

    let mut saw_batch = false;
    let report = Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        qdrant: &qdrant,
        port: &port,
        card: &card,
    }
    .publish_observed(&kernel.chunk_set, None, &mut |progress| {
        assert_eq!(progress.generation, 2);
        assert_eq!(
            backend.cached_alias(&alias).as_deref(),
            Some(old_collection.as_str())
        );
        saw_batch = true;
        ControlFlow::Continue(())
    })
    .await
    .unwrap();
    assert!(saw_batch);
    assert_eq!(report.generation, 2);
    assert_eq!(report.retired, Some(legacy.id));
    assert_eq!(
        backend.alias(&alias).await.as_deref(),
        Some(report.qdrant_collection.as_str())
    );
    assert!(backend.exists(&old_collection).await);
    backend
        .cleanup(&kernel.collection, [legacy.id, report.generation])
        .await;
}

#[tokio::test]
async fn a_generation_builds_in_its_own_collection_then_takes_the_alias() {
    for backend in backends("a_generation_builds_in_its_own_collection_then_takes_the_alias") {
        builds_in_its_own_collection(&backend).await;
    }
}

/// One guide gives four chunks: two batches, of 3 and 1.
async fn builds_in_its_own_collection(backend: &Backend) {
    let kernel = Kernel::with_guides(1);
    let (card, port) = (embedder(8), Embedder::default());
    let qdrant = backend.client();
    let projection =
        projection(&kernel, &qdrant, &port, &card).with_batch_size(NonZeroUsize::new(3).unwrap());
    let report = projection.publish(&kernel.chunk_set).await.unwrap();
    let collection = collection_of(&kernel, 1);
    let profile = format!("dense/1:sha256:{}", card.digest().as_str());
    let expected = Report {
        collection: kernel.collection.clone(),
        chunk_set: kernel.chunk_set.clone(),
        generation: 1,
        qdrant_collection: collection.clone(),
        alias: alias_of(&kernel),
        points: 4,
        embedding_profile: profile.clone(),
        sparse_profile: "bm25-en-fr/1".to_owned(),
        retired: None,
    };
    assert_eq!(report, expected, "{}", backend.name);
    holds_its_points_behind_the_alias(backend, &kernel, &collection).await;
    records_its_profiles(&kernel, &profile);
    let batches: Vec<usize> = port.calls().iter().map(Vec::len).collect();
    assert_eq!(batches, [3, 1]);
    assert_eq!(port.rooms(), [Room::Free, Room::Free]);
    backend.cleanup(&kernel.collection, 1..=1).await;
}

/// `collection`, the first generation of `kernel`, has a dense vector of 8
/// dimensions compared by cosine and a sparse vector weighted by IDF, holds
/// its 4 points, and takes the alias.
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
    assert_eq!(backend.ef_construct(collection).await, Some(200));
    assert_eq!(backend.count(collection).await, 4);
    let alias = backend.alias(&alias_of(kernel)).await;
    assert_eq!(alias.as_deref(), Some(collection));
}

/// The first generation of `kernel` is published, with its 4 points, the
/// dense profile `profile` and the analyzer's sparse profile.
fn records_its_profiles(kernel: &Kernel, profile: &str) {
    let recorded = kernel
        .database
        .generation(&kernel.scopes, 1)
        .unwrap()
        .unwrap();
    assert_eq!(recorded.state, GenerationState::Published);
    assert_eq!(recorded.point_count, Some(4));
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
    let cases = [
        (
            "chunk-0-lead",
            json!([]),
            json!(["workspace/default", scope, docs]),
            json!(VERSION),
        ),
        (
            "chunk-0-1",
            json!(["Guide 0", "Retries"]),
            json!(["workspace/default", scope, docs]),
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
            "identifiers": [],
            "identifier_profile": IDENTIFIER_PROFILE,
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
    backend.cleanup(&kernel.collection, 1..=1).await;
}

#[tokio::test]
async fn the_same_chunk_set_gives_the_same_point_ids() {
    for backend in backends("the_same_chunk_set_gives_the_same_point_ids") {
        let (first, second) = gives_the_same_point_ids(&backend).await;
        backend.cleanup(&first, 1..=1).await;
        backend.cleanup(&second, 1..=1).await;
    }
}

/// Two kernels whose chunk sets hold the same chunks, in collections of
/// their own; then the first published again.
async fn gives_the_same_point_ids(backend: &Backend) -> (String, String) {
    let (first, second) = (Kernel::with_guides(3), Kernel::with_guides(3));
    let (card, port) = (embedder(8), Embedder::default());
    let report = publish(&first, backend, &port, &card).await.unwrap();
    let other = publish(&second, backend, &port, &card).await.unwrap();
    let ids = backend.ids(&report.qdrant_collection).await;
    assert_eq!(ids.len(), 10);
    assert_eq!(backend.ids(&other.qdrant_collection).await, ids);
    assert_sample_ids(&ids, backend.name);
    let calls = port.calls().len();
    let again = publish(&first, backend, &port, &card).await.unwrap();
    assert_eq!(again, report);
    assert_eq!(
        port.calls().len(),
        calls,
        "a published generation embeds nothing"
    );
    assert_eq!(backend.ids(&report.qdrant_collection).await, ids);
    (first.collection.clone(), second.collection.clone())
}

fn assert_sample_ids(ids: &BTreeSet<String>, backend: &str) {
    for chunk in ["chunk-0-1", "chunk-0-lead", "chunk-1-2"] {
        assert!(ids.contains(point_id(chunk)), "{backend} {chunk}");
    }
}
