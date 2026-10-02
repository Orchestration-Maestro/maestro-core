//! Synthetic transport double for the existing retrieval port, not a second store.

use crate::index::{
    CollectionLayout, PayloadFieldKind, PointHit, ProjectionCursor, ProjectionError,
    ProjectionFilter, ProjectionPage, ProjectionPoint, RetrievalProjectionPort, SparseValues,
};
use std::{collections::BTreeMap, sync::Mutex};

/// One disposable collection and captured backend requests.
#[derive(Debug, Default)]
pub(super) struct Backend {
    /// Current physical collection and layout.
    pub(super) layout: Mutex<Option<(String, CollectionLayout)>>,
    /// Applied points, preserving vectors and canonical payloads.
    pub(super) points: Mutex<BTreeMap<String, ProjectionPoint>>,
    /// Last request filters, applied before top-k.
    pub(super) filters: Mutex<Vec<ProjectionFilter>>,
}

impl Backend {
    /// Delete only synthetic disposable storage; real authority remains separate.
    pub(super) fn delete(&self) {
        self.layout.lock().unwrap().take();
        self.points.lock().unwrap().clear();
    }
}

/// Execute the exact filter semantics emitted by the real adapter.
fn matches(point: &ProjectionPoint, filter: &ProjectionFilter) -> bool {
    match filter {
        ProjectionFilter::ExactString { field, value } => {
            point.payload.get(field).and_then(serde_json::Value::as_str) == Some(value.as_str())
        }
        ProjectionFilter::All(filters) => filters.iter().all(|filter| matches(point, filter)),
        ProjectionFilter::AnyString { .. } => {
            panic!("descriptor adapter must emit exact scoped filters")
        }
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "The in-memory transport double has no I/O to await."
)]
impl RetrievalProjectionPort for Backend {
    async fn collection_exists(&self, collection: &str) -> Result<bool, ProjectionError> {
        Ok(self
            .layout
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(name, _)| name == collection))
    }
    async fn create_collection(
        &self,
        collection: &str,
        layout: CollectionLayout,
    ) -> Result<(), ProjectionError> {
        *self.layout.lock().unwrap() = Some((collection.into(), layout));
        Ok(())
    }
    async fn collection_layout(
        &self,
        collection: &str,
    ) -> Result<Option<CollectionLayout>, ProjectionError> {
        Ok(self
            .layout
            .lock()
            .unwrap()
            .as_ref()
            .filter(|(name, _)| name == collection)
            .map(|(_, layout)| layout.clone()))
    }
    async fn index_payload_fields(&self, _collection: &str) -> Result<(), ProjectionError> {
        Ok(())
    }
    async fn payload_fields(
        &self,
        _collection: &str,
    ) -> Result<BTreeMap<String, PayloadFieldKind>, ProjectionError> {
        Err(ProjectionError::new("unsupported test transport call"))
    }
    async fn upsert_points(
        &self,
        _collection: &str,
        points: Vec<ProjectionPoint>,
    ) -> Result<(), ProjectionError> {
        self.points
            .lock()
            .unwrap()
            .extend(points.into_iter().map(|point| (point.id.clone(), point)));
        Ok(())
    }
    async fn count_points(&self, _collection: &str) -> Result<u64, ProjectionError> {
        Ok(u64::try_from(self.points.lock().unwrap().len()).unwrap())
    }
    async fn point_ids(
        &self,
        _collection: &str,
        _ids: &[String],
    ) -> Result<Vec<String>, ProjectionError> {
        Err(ProjectionError::new("unsupported test transport call"))
    }
    async fn payloads(
        &self,
        _collection: &str,
        ids: &[String],
    ) -> Result<Vec<PointHit>, ProjectionError> {
        let points = self.points.lock().unwrap();
        Ok(ids
            .iter()
            .filter_map(|id| points.get(id))
            .map(|point| PointHit {
                id: point.id.clone(),
                score: None,
                payload: point.payload.clone(),
            })
            .collect())
    }
    async fn search_dense(
        &self,
        collection: &str,
        _vector: Vec<f32>,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, ProjectionError> {
        if !self.collection_exists(collection).await? {
            return Err(ProjectionError::new("unknown collection"));
        }
        self.filters.lock().unwrap().push(filter.clone());
        Ok(self
            .points
            .lock()
            .unwrap()
            .values()
            .filter(|point| matches(point, &filter))
            .take(limit)
            .map(|point| PointHit {
                id: point.id.clone(),
                score: Some(1.0),
                payload: point.payload.clone(),
            })
            .collect())
    }
    async fn search_sparse(
        &self,
        _collection: &str,
        _vector: SparseValues,
        _limit: usize,
        _filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, ProjectionError> {
        Err(ProjectionError::new("unsupported test transport call"))
    }
    async fn scroll(
        &self,
        _collection: &str,
        _filter: ProjectionFilter,
        _cursor: Option<ProjectionCursor>,
    ) -> Result<ProjectionPage, ProjectionError> {
        Err(ProjectionError::new("unsupported test transport call"))
    }
    async fn alias_target(&self, _alias: &str) -> Result<Option<String>, ProjectionError> {
        Err(ProjectionError::new("unsupported test transport call"))
    }
    async fn replace_alias(&self, _alias: &str, _collection: &str) -> Result<(), ProjectionError> {
        Err(ProjectionError::new("unsupported test transport call"))
    }
}
