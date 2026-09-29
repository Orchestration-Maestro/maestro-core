//! Publication and standalone verification through a transport-independent projection fake.

use super::{
    kernel::Kernel,
    models::{Embedder, embedder},
};
use maestro_knowledge::{
    index::{
        CollectionLayout, PointHit, Projection, ProjectionFilter, ProjectionPage, ProjectionPoint,
        QdrantError, RebuildGuard, RetrievalProjectionPort, SparseValues,
    },
    publish::verify_generation,
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::ControlFlow,
    sync::Mutex,
};

#[derive(Debug, Default)]
struct FakePort {
    collections: Mutex<HashMap<String, FakeCollection>>,
    aliases: Mutex<HashMap<String, String>>,
}

#[derive(Debug, Default)]
struct FakeCollection {
    layout: Option<CollectionLayout>,
    points: HashMap<String, PointHit>,
    fields: BTreeMap<String, String>,
}

impl FakePort {
    fn error() -> QdrantError {
        QdrantError::InvalidAnswer("fake port operation is unsupported".to_owned())
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "The in-memory fake has no I/O to await."
)]
impl RetrievalProjectionPort for FakePort {
    type Error = QdrantError;

    async fn collection_exists(&self, name: &str) -> Result<bool, Self::Error> {
        Ok(self.collections.lock().unwrap().contains_key(name))
    }
    async fn create_collection(
        &self,
        name: &str,
        layout: CollectionLayout,
    ) -> Result<(), Self::Error> {
        self.collections.lock().unwrap().insert(
            name.to_owned(),
            FakeCollection {
                layout: Some(layout),
                ..FakeCollection::default()
            },
        );
        Ok(())
    }
    async fn collection_layout(&self, name: &str) -> Result<Option<CollectionLayout>, Self::Error> {
        Ok(self
            .collections
            .lock()
            .unwrap()
            .get(name)
            .and_then(|collection| collection.layout.clone()))
    }
    async fn index_payload_fields(&self, name: &str) -> Result<(), Self::Error> {
        let mut collections = self.collections.lock().unwrap();
        let collection = collections.get_mut(name).ok_or_else(Self::error)?;
        collection.fields.extend(
            ["scope_tags", "identifiers", "identifier_profile", "version"]
                .map(|field| (field.to_owned(), "keyword".to_owned())),
        );
        Ok(())
    }
    async fn payload_fields(&self, name: &str) -> Result<BTreeMap<String, String>, Self::Error> {
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
    ) -> Result<(), Self::Error> {
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
    async fn count_points(&self, name: &str) -> Result<u64, Self::Error> {
        self.collections
            .lock()
            .unwrap()
            .get(name)
            .map(|collection| u64::try_from(collection.points.len()).unwrap_or(u64::MAX))
            .ok_or_else(Self::error)
    }
    async fn point_ids(&self, name: &str, ids: &[String]) -> Result<Vec<String>, Self::Error> {
        let collections = self.collections.lock().unwrap();
        let collection = collections.get(name).ok_or_else(Self::error)?;
        Ok(ids
            .iter()
            .filter(|id| collection.points.contains_key(*id))
            .cloned()
            .collect())
    }
    async fn payloads(&self, name: &str, ids: &[String]) -> Result<Vec<PointHit>, Self::Error> {
        let collections = self.collections.lock().unwrap();
        let collection = collections.get(name).ok_or_else(Self::error)?;
        Ok(ids
            .iter()
            .filter_map(|id| collection.points.get(id).cloned())
            .collect())
    }
    async fn search_dense(
        &self,
        _: &str,
        _: Vec<f32>,
        _: usize,
        _: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error> {
        Ok(Vec::new())
    }
    async fn search_sparse(
        &self,
        _: &str,
        _: SparseValues,
        _: usize,
        _: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error> {
        Ok(Vec::new())
    }
    async fn scroll(
        &self,
        _: &str,
        _: ProjectionFilter,
        _: Option<String>,
    ) -> Result<ProjectionPage, Self::Error> {
        Ok(ProjectionPage {
            points: Vec::new(),
            next: None,
        })
    }
    async fn alias_target(&self, alias: &str) -> Result<Option<String>, Self::Error> {
        Ok(self.aliases.lock().unwrap().get(alias).cloned())
    }
    async fn replace_alias(&self, alias: &str, collection: &str) -> Result<(), Self::Error> {
        if !self.collections.lock().unwrap().contains_key(collection) {
            return Err(Self::error());
        }
        self.aliases
            .lock()
            .unwrap()
            .insert(alias.to_owned(), collection.to_owned());
        Ok(())
    }
    async fn remove_collection(&self, name: &str) -> Result<(), Self::Error> {
        self.collections.lock().unwrap().remove(name);
        Ok(())
    }
}

#[tokio::test]
async fn publication_and_standalone_verification_use_the_fake_port() {
    let kernel = Kernel::with_guides(1);
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
}
