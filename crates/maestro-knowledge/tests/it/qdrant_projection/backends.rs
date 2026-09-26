//! The Qdrant servers a test runs against, and what it reads back from them
//! through the official client. Every test runs against the fake; when
//! `MAESTRO_QDRANT_URL` names the gRPC API of a real Qdrant 1.19, such as
//! `http://127.0.0.1:6334`, the tests that hold for Qdrant itself run against
//! it too, and say so on stderr when it is unset. `integration.yml` runs them
//! against the pinned image in CI:
//!
//! ```sh
//! MAESTRO_QDRANT_URL=http://127.0.0.1:6334 \
//!   cargo test -p maestro-knowledge --test it qdrant_projection
//! ```

use super::fake::FakeQdrant;
use maestro_knowledge::index::Qdrant;
use qdrant_client::{
    Payload, Qdrant as Client,
    qdrant::{
        CollectionParams, CountPointsBuilder, CreateCollectionBuilder, DeletePointsBuilder,
        Distance, GetPointsBuilder, Modifier, NamedVectors, PointId, PointStruct, PointsIdsList,
        RetrievedPoint, ScrollPointsBuilder, SparseVectorParamsBuilder, SparseVectorsConfigBuilder,
        UpsertPointsBuilder, Vector, VectorParamsBuilder, VectorsConfigBuilder,
        point_id::PointIdOptions,
    },
};
use serde_json::json;
use std::{collections::BTreeSet, env};

/// The variable that names a real Qdrant.
const VARIABLE: &str = "MAESTRO_QDRANT_URL";

/// A Qdrant server a test runs against.
#[derive(Debug)]
pub(super) struct Backend {
    /// Which one: `fake` or `qdrant`.
    pub(super) name: &'static str,
    /// Where its gRPC API answers.
    pub(super) url: String,
    /// The fake, when it is one.
    pub(super) fake: Option<FakeQdrant>,
}

/// The fake alone, served by the calling test's runtime: for what only the
/// fake can show.
pub(super) fn fake() -> Backend {
    let fake = FakeQdrant::serve();
    Backend {
        name: "fake",
        url: fake.url().to_owned(),
        fake: Some(fake),
    }
}

/// The servers the test `test` runs against: the fake, then the Qdrant
/// `MAESTRO_QDRANT_URL` names, if it names one; else `test` says on stderr
/// that its leg against a real Qdrant is skipped.
pub(super) fn backends(test: &str) -> Vec<Backend> {
    let mut backends = vec![fake()];
    match env::var(VARIABLE) {
        Ok(url) => backends.push(Backend {
            name: "qdrant",
            url,
            fake: None,
        }),
        Err(_) => eprintln!(
            "{test}: {VARIABLE} is unset, so the test runs against the fake Qdrant only and its \
             leg against a real Qdrant is skipped; see tests/it/qdrant_projection/backends.rs"
        ),
    }
    backends
}

impl Backend {
    /// The library's client of the server.
    pub(super) fn client(&self) -> Qdrant {
        Qdrant::new(&self.url).unwrap()
    }

    /// The official client, to read the server with.
    fn inspector(&self) -> Client {
        Client::from_url(&self.url)
            .skip_compatibility_check()
            .build()
            .unwrap()
    }

    /// The parameters of `collection`.
    pub(super) async fn parameters(&self, collection: &str) -> CollectionParams {
        let info = self.inspector().collection_info(collection).await.unwrap();
        info.result.unwrap().config.unwrap().params.unwrap()
    }

    /// How many points `collection` holds.
    pub(super) async fn count(&self, collection: &str) -> u64 {
        let request = CountPointsBuilder::new(collection).exact(true);
        let counted = self.inspector().count(request).await.unwrap();
        counted.result.unwrap().count
    }

    /// The ID of every point of `collection`.
    pub(super) async fn ids(&self, collection: &str) -> BTreeSet<String> {
        let request = ScrollPointsBuilder::new(collection)
            .limit(10_000)
            .with_payload(false)
            .with_vectors(false);
        let page = self.inspector().scroll(request).await.unwrap();
        assert_eq!(page.next_page_offset, None);
        page.result
            .into_iter()
            .map(|point| match point.id.unwrap().point_id_options.unwrap() {
                PointIdOptions::Uuid(uuid) => uuid,
                PointIdOptions::Num(number) => number.to_string(),
            })
            .collect()
    }

    /// The point `id` of `collection`, with its payload and vectors.
    pub(super) async fn point(&self, collection: &str, id: &str) -> RetrievedPoint {
        let request = GetPointsBuilder::new(collection, vec![PointId::from(id)])
            .with_payload(true)
            .with_vectors(true);
        let mut found = self.inspector().get_points(request).await.unwrap().result;
        assert_eq!(found.len(), 1, "{found:?}");
        found.remove(0)
    }

    /// The collection the alias `alias` points at, if it exists.
    pub(super) async fn alias(&self, alias: &str) -> Option<String> {
        let aliases = self.inspector().list_aliases().await.unwrap().aliases;
        aliases
            .into_iter()
            .find(|entry| entry.alias_name == alias)
            .map(|entry| entry.collection_name)
    }

    /// Whether `collection` exists.
    pub(super) async fn exists(&self, collection: &str) -> bool {
        self.inspector()
            .collection_exists(collection)
            .await
            .unwrap()
    }

    /// Creates `collection` with a dense vector `dense` of `dimensions`
    /// compared by `distance` and the sparse vector `bm25` weighted by
    /// `modifier`, if any, as no publication did.
    pub(super) async fn create(
        &self,
        collection: &str,
        dimensions: u64,
        distance: Distance,
        modifier: Option<Modifier>,
    ) {
        let mut dense = VectorsConfigBuilder::default();
        dense.add_named_vector_params("dense", VectorParamsBuilder::new(dimensions, distance));
        let params = modifier.map_or_else(SparseVectorParamsBuilder::default, |modifier| {
            SparseVectorParamsBuilder::default().modifier(modifier)
        });
        let mut sparse = SparseVectorsConfigBuilder::default();
        sparse.add_named_vector_params("bm25", params);
        let create = CreateCollectionBuilder::new(collection)
            .vectors_config(dense)
            .sparse_vectors_config(sparse);
        self.inspector().create_collection(create).await.unwrap();
    }

    /// Writes a point no chunk has into `collection`, whose dense vector has
    /// `dimensions`.
    pub(super) async fn intrude(&self, collection: &str, dimensions: usize) {
        let vectors = NamedVectors::default()
            .add_vector("dense", vec![1.0; dimensions])
            .add_vector("bm25", Vector::new_sparse(vec![7], vec![1.0]));
        let payload = Payload::try_from(json!({ "chunk_id": "not-a-chunk" })).unwrap();
        let point = PointStruct::new("00000000-0000-4000-8000-000000000001", vectors, payload);
        let upsert = UpsertPointsBuilder::new(collection, vec![point]).wait(true);
        self.inspector().upsert_points(upsert).await.unwrap();
    }

    /// Deletes the point `id` of `collection`.
    pub(super) async fn delete(&self, collection: &str, id: &str) {
        let ids = PointsIdsList {
            ids: vec![PointId::from(id)],
        };
        let delete = DeletePointsBuilder::new(collection).points(ids).wait(true);
        self.inspector().delete_points(delete).await.unwrap();
    }
}
