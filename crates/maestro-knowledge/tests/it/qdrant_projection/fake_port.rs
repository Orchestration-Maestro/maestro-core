//! Publication and standalone verification through a transport-independent projection fake.

use super::{
    kernel::Kernel,
    models::{Embedder, embedder},
};
use maestro_knowledge::{
    index::{
        CollectionLayout, PointHit, Projection, ProjectionCursor, ProjectionError,
        ProjectionFilter, ProjectionPage, ProjectionPoint, RebuildGuard, RetrievalProjectionPort,
        SparseValues,
    },
    publish::verify_generation,
    query::understand,
    search::{
        Query,
        routes::{
            dense::{Embedder as SearchEmbedder, search_dense},
            identifier::search_identifiers,
            lexical::search_bm25,
        },
    },
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::ControlFlow,
    sync::Mutex,
    time::Duration,
};
use tokio::time::Instant;

#[derive(Debug, Default)]
pub(super) struct FakePort {
    pub(super) collections: Mutex<HashMap<String, FakeCollection>>,
    pub(super) aliases: Mutex<HashMap<String, String>>,
    fail_on: Mutex<Option<(&'static str, usize)>>,
}

#[derive(Debug, Default)]
pub(super) struct FakeCollection {
    pub(super) layout: Option<CollectionLayout>,
    pub(super) points: HashMap<String, PointHit>,
    pub(super) fields: BTreeMap<String, String>,
}

impl FakePort {
    fn error() -> ProjectionError {
        ProjectionError::new("fake port operation is unsupported")
    }

    fn fail(&self, operation: &'static str) {
        *self.fail_on.lock().unwrap() = Some((operation, 0));
    }

    pub(super) fn fail_after(&self, operation: &'static str, successful_calls: usize) {
        *self.fail_on.lock().unwrap() = Some((operation, successful_calls));
    }

    fn maybe_fail(&self, operation: &'static str) -> Result<(), ProjectionError> {
        let mut failure = self.fail_on.lock().unwrap();
        if let Some((target, remaining)) = failure.as_mut()
            && *target == operation
        {
            if *remaining == 0 {
                *failure = None;
                return Err(ProjectionError::new(format!("fake refused {operation}")));
            }
            *remaining -= 1;
        }
        Ok(())
    }

    pub(super) fn drop_point(&self, collection: &str, id: &str) {
        if let Some(collection) = self.collections.lock().unwrap().get_mut(collection) {
            collection.points.remove(id);
        }
    }

    fn clear_failure(&self) {
        *self.fail_on.lock().unwrap() = None;
    }

    fn search_hits(
        &self,
        name: &str,
        filter: &ProjectionFilter,
        limit: usize,
    ) -> Result<Vec<PointHit>, ProjectionError> {
        let collections = self.collections.lock().unwrap();
        let collection = collections.get(name).ok_or_else(Self::error)?;
        Ok(collection
            .points
            .values()
            .filter(|point| matches_filter(point, filter))
            .take(limit)
            .cloned()
            .map(|mut point| {
                point.score = Some(0.75);
                point
            })
            .collect())
    }
}

fn matches_filter(point: &PointHit, filter: &ProjectionFilter) -> bool {
    match filter {
        ProjectionFilter::AnyString { field, values } => point
            .payload
            .get(field)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|actual| {
                actual
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .any(|actual| values.iter().any(|value| value == actual))
            }),
        ProjectionFilter::ExactString { field, value } => {
            point.payload.get(field).and_then(serde_json::Value::as_str) == Some(value)
        }
        ProjectionFilter::All(filters) => {
            filters.iter().all(|filter| matches_filter(point, filter))
        }
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "The in-memory fake has no I/O to await."
)]
impl RetrievalProjectionPort for FakePort {
    async fn collection_exists(&self, name: &str) -> Result<bool, ProjectionError> {
        Ok(self.collections.lock().unwrap().contains_key(name))
    }
    async fn create_collection(
        &self,
        name: &str,
        layout: CollectionLayout,
    ) -> Result<(), ProjectionError> {
        self.collections.lock().unwrap().insert(
            name.to_owned(),
            FakeCollection {
                layout: Some(layout),
                ..FakeCollection::default()
            },
        );
        Ok(())
    }
    async fn collection_layout(
        &self,
        name: &str,
    ) -> Result<Option<CollectionLayout>, ProjectionError> {
        Ok(self
            .collections
            .lock()
            .unwrap()
            .get(name)
            .and_then(|collection| collection.layout.clone()))
    }
    async fn index_payload_fields(&self, name: &str) -> Result<(), ProjectionError> {
        let mut collections = self.collections.lock().unwrap();
        let collection = collections.get_mut(name).ok_or_else(Self::error)?;
        collection.fields.extend(
            ["scope_tags", "identifiers", "identifier_profile", "version"]
                .map(|field| (field.to_owned(), "keyword".to_owned())),
        );
        Ok(())
    }
    async fn payload_fields(
        &self,
        name: &str,
    ) -> Result<BTreeMap<String, String>, ProjectionError> {
        self.collections
            .lock()
            .unwrap()
            .get(name)
            .map(|collection| collection.fields.clone())
            .ok_or_else(Self::error)
    }
    async fn upsert_points(
        &self,
        name: &str,
        points: Vec<ProjectionPoint>,
    ) -> Result<(), ProjectionError> {
        self.maybe_fail("upsert_points")?;
        let mut collections = self.collections.lock().unwrap();
        let collection = collections.get_mut(name).ok_or_else(Self::error)?;
        for point in points {
            collection.points.insert(
                point.id.clone(),
                PointHit {
                    id: point.id,
                    score: None,
                    payload: point.payload,
                },
            );
        }
        Ok(())
    }
    async fn count_points(&self, name: &str) -> Result<u64, ProjectionError> {
        self.collections
            .lock()
            .unwrap()
            .get(name)
            .map(|collection| u64::try_from(collection.points.len()).unwrap_or(u64::MAX))
            .ok_or_else(Self::error)
    }
    async fn point_ids(&self, name: &str, ids: &[String]) -> Result<Vec<String>, ProjectionError> {
        let collections = self.collections.lock().unwrap();
        let collection = collections.get(name).ok_or_else(Self::error)?;
        Ok(ids
            .iter()
            .filter(|id| collection.points.contains_key(*id))
            .cloned()
            .collect())
    }
    async fn payloads(&self, name: &str, ids: &[String]) -> Result<Vec<PointHit>, ProjectionError> {
        let collections = self.collections.lock().unwrap();
        let collection = collections.get(name).ok_or_else(Self::error)?;
        Ok(ids
            .iter()
            .filter_map(|id| collection.points.get(id).cloned())
            .collect())
    }
    async fn search_dense(
        &self,
        collection: &str,
        _: Vec<f32>,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, ProjectionError> {
        self.search_hits(collection, &filter, limit)
    }
    async fn search_sparse(
        &self,
        collection: &str,
        _: SparseValues,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, ProjectionError> {
        self.search_hits(collection, &filter, limit)
    }
    async fn scroll(
        &self,
        collection: &str,
        filter: ProjectionFilter,
        _: Option<ProjectionCursor>,
    ) -> Result<ProjectionPage, ProjectionError> {
        Ok(ProjectionPage {
            points: self.search_hits(collection, &filter, 64)?,
            next: None,
        })
    }
    async fn alias_target(&self, alias: &str) -> Result<Option<String>, ProjectionError> {
        Ok(self.aliases.lock().unwrap().get(alias).cloned())
    }
    async fn replace_alias(&self, alias: &str, collection: &str) -> Result<(), ProjectionError> {
        if !self.collections.lock().unwrap().contains_key(collection) {
            return Err(Self::error());
        }
        self.aliases
            .lock()
            .unwrap()
            .insert(alias.to_owned(), collection.to_owned());
        Ok(())
    }
}

#[test]
fn fake_search_enforces_scope_version_filters_and_limit() {
    let fake = FakePort::default();
    fake.fail("upsert_points");
    assert!(fake.maybe_fail("upsert_points").is_err());
    fake.clear_failure();
    fake.collections.lock().unwrap().insert(
        "collection".to_owned(),
        FakeCollection {
            points: [
                (
                    "allowed".to_owned(),
                    PointHit {
                        id: "allowed".to_owned(),
                        score: None,
                        payload: [
                            ("scope_tags".to_owned(), serde_json::json!(["read"])),
                            ("version".to_owned(), serde_json::json!("v1")),
                        ]
                        .into(),
                    },
                ),
                (
                    "outside".to_owned(),
                    PointHit {
                        id: "outside".to_owned(),
                        score: None,
                        payload: [
                            ("scope_tags".to_owned(), serde_json::json!(["admin"])),
                            ("version".to_owned(), serde_json::json!("v1")),
                        ]
                        .into(),
                    },
                ),
            ]
            .into(),
            ..FakeCollection::default()
        },
    );
    let hits = fake
        .search_hits(
            "collection",
            &ProjectionFilter::All(vec![
                ProjectionFilter::AnyString {
                    field: "scope_tags".to_owned(),
                    values: vec!["read".to_owned()],
                },
                ProjectionFilter::ExactString {
                    field: "version".to_owned(),
                    value: "v1".to_owned(),
                },
            ]),
            1,
        )
        .unwrap();
    assert_eq!(
        hits.iter().map(|hit| hit.id.as_str()).collect::<Vec<_>>(),
        ["allowed"]
    );
}

#[tokio::test]
async fn publication_and_standalone_verification_use_the_fake_port() {
    let kernel = Kernel::with_changed_guides(1, &|kernel, guide, mut chunks| {
        if guide == 0 {
            chunks[0].digest = kernel.put(b"The command handles ERR-042 safely.");
        }
        chunks
    });
    let fake = FakePort::default();
    let model_port = Embedder::default();
    let card = embedder(8);
    let report = Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        projection: &fake,
        port: &model_port,
        card: &card,
    }
    .publish(&kernel.chunk_set)
    .await
    .unwrap();

    let result = verify_generation(&kernel.database, &kernel.scopes, &fake, report.generation)
        .await
        .unwrap();
    assert!(result.findings.is_empty(), "{:#?}", result.findings);

    let rebuilt = Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        projection: &fake,
        port: &model_port,
        card: &card,
    }
    .republish_observed(
        &kernel.chunk_set,
        RebuildGuard {
            expected_published: Some(report.generation),
            generation_watermark: report.generation,
        },
        None,
        &mut |_| ControlFlow::Continue(()),
    )
    .await
    .unwrap();
    assert_ne!(rebuilt.generation, report.generation);
    let verified = verify_generation(&kernel.database, &kernel.scopes, &fake, rebuilt.generation)
        .await
        .unwrap();
    assert!(verified.findings.is_empty(), "{:#?}", verified.findings);

    let generation = kernel
        .database
        .generation(&kernel.scopes, rebuilt.generation)
        .unwrap()
        .unwrap();
    let query = Query {
        generation: &generation,
        scopes: &kernel.scopes,
        text: "guide",
        limit: 5,
        version: None,
        qdrant: &fake,
    };
    let hits = search_bm25(&query).await.unwrap();
    assert!(!hits.is_empty());
    let search_embedder = SearchEmbedder {
        port: &model_port,
        card: &card,
    };
    assert!(
        !search_dense(&query, &search_embedder)
            .await
            .unwrap()
            .is_empty()
    );

    let understood = understand("ERR-042");
    let identifier_query = Query {
        text: "ERR-042",
        ..query
    };
    let identifiers = search_identifiers(
        &identifier_query,
        kernel.database.clone(),
        &understood,
        Instant::now() + Duration::from_secs(2),
    )
    .await;
    assert!(!identifiers.hits.is_empty());

    assert_dropped_point_is_reported(&fake, &kernel, &report.qdrant_collection, report.generation)
        .await;
}

/// Confirms a missing fake point is reported as a projection count mismatch.
async fn assert_dropped_point_is_reported(
    fake: &FakePort,
    kernel: &Kernel,
    collection: &str,
    generation: i64,
) {
    let id = fake
        .collections
        .lock()
        .unwrap()
        .get(collection)
        .and_then(|collection| collection.points.keys().next().cloned())
        .unwrap();
    fake.drop_point(collection, &id);
    let verification = verify_generation(&kernel.database, &kernel.scopes, fake, generation)
        .await
        .unwrap();
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains("point count mismatch"))
    );
}
