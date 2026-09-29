//! Backend-neutral operations and values for a generation's retrieval projection.

use serde_json::Value;
use std::{collections::BTreeMap, error, fmt};

/// The named vector layout required by current dense and sparse retrieval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionLayout {
    /// The dimension count of the named dense vector.
    pub dense_dimensions: u64,
    /// Whether the named dense vector is present.
    pub dense_present: bool,
    /// The backend-neutral name of its distance function.
    pub dense_distance: String,
    /// Whether the named BM25 sparse vector is present.
    pub sparse_present: bool,
    /// Its modifier name, or absent when no modifier is set.
    pub sparse_modifier: Option<String>,
}

/// A point ready to store in a generation's projection.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionPoint {
    /// Stable physical point identifier.
    pub id: String,
    /// Dense vector values.
    pub dense: Vec<f32>,
    /// Sparse vector token identifiers and weights.
    pub sparse: SparseValues,
    /// Backend-neutral point payload.
    pub payload: BTreeMap<String, Value>,
}

/// Sparse token identifiers and weights.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SparseValues {
    /// Sorted unique token identifiers.
    pub indices: Vec<u32>,
    /// Weights corresponding to `indices`.
    pub values: Vec<f32>,
}

/// A payload or scored point returned by the projection.
#[derive(Clone, Debug, PartialEq)]
pub struct PointHit {
    /// Stable physical point identifier.
    pub id: String,
    /// Search score, absent for payload-only reads.
    pub score: Option<f64>,
    /// Backend-neutral point payload.
    pub payload: BTreeMap<String, Value>,
}

/// Supported backend-independent filter predicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionFilter {
    /// Match points whose string-array field contains any of the values.
    AnyString {
        /// Payload field to inspect.
        field: String,
        /// Values accepted in the string-array field.
        values: Vec<String>,
    },
    /// Match points whose string field equals the value.
    ExactString {
        /// Payload field to inspect.
        field: String,
        /// Required exact string value.
        value: String,
    },
    /// Require every child predicate.
    All(Vec<Self>),
}

/// A backend-neutral cursor for stable numeric or textual point IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionCursor {
    /// Numeric point ID.
    Number(u64),
    /// Text point ID, including UUIDs.
    Text(String),
}

/// One backend-neutral page of identifier-route payloads.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionPage {
    /// Payloads in backend-returned order.
    pub points: Vec<PointHit>,
    /// Cursor for the next page, absent when exhausted.
    pub next: Option<ProjectionCursor>,
}

/// A transport-independent projection backend failure.
#[derive(Debug)]
pub struct ProjectionError {
    /// Stable operation failure message.
    message: String,
    /// The adapter's underlying cause, when one exists.
    source: Option<Box<dyn error::Error + Send + Sync>>,
}

impl ProjectionError {
    /// Creates an operational error without a lower-level cause.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    /// Creates an operational error while preserving its adapter cause.
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl ProjectionError {
    /// Consumes the error for adapter-specific compatibility mapping.
    pub(super) fn into_parts(self) -> (String, Option<Box<dyn error::Error + Send + Sync>>) {
        (self.message, self.source)
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl error::Error for ProjectionError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}

/// Operations used to publish, verify, rebuild, and search a generation.
#[expect(
    async_fn_in_trait,
    reason = "The port deliberately exposes asynchronous operations for its local adapters."
)]
pub trait RetrievalProjectionPort: fmt::Debug {
    /// The adapter's operational error, with no transport client types.
    type Error: error::Error + Send + Sync + 'static;

    /// Whether a physical collection exists.
    async fn collection_exists(&self, collection: &str) -> Result<bool, Self::Error>;
    /// Create a physical collection with the current named vector layout.
    async fn create_collection(
        &self,
        collection: &str,
        layout: CollectionLayout,
    ) -> Result<(), Self::Error>;
    /// Read the current named vector layout, or `None` when absent.
    async fn collection_layout(
        &self,
        collection: &str,
    ) -> Result<Option<CollectionLayout>, Self::Error>;
    /// Create keyword indexes for supported payload fields.
    async fn index_payload_fields(&self, collection: &str) -> Result<(), Self::Error>;
    /// Read indexed payload field names and their backend-neutral kinds.
    async fn payload_fields(
        &self,
        collection: &str,
    ) -> Result<BTreeMap<String, String>, Self::Error>;
    /// Upsert points and wait until they have been applied.
    async fn upsert_points(
        &self,
        collection: &str,
        points: Vec<ProjectionPoint>,
    ) -> Result<(), Self::Error>;
    /// Count points exactly.
    async fn count_points(&self, collection: &str) -> Result<u64, Self::Error>;
    /// Return IDs from `ids` which currently exist.
    async fn point_ids(&self, collection: &str, ids: &[String])
    -> Result<Vec<String>, Self::Error>;
    /// Read payloads for the requested physical point IDs.
    async fn payloads(
        &self,
        collection: &str,
        ids: &[String],
    ) -> Result<Vec<PointHit>, Self::Error>;
    /// Query the named dense route with an authorization filter.
    async fn search_dense(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error>;
    /// Query the named BM25 route with an authorization filter.
    async fn search_sparse(
        &self,
        collection: &str,
        vector: SparseValues,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error>;
    /// Read one page for the exact identifier route.
    async fn scroll(
        &self,
        collection: &str,
        filter: ProjectionFilter,
        cursor: Option<ProjectionCursor>,
    ) -> Result<ProjectionPage, Self::Error>;
    /// Resolve the collection named by an alias.
    async fn alias_target(&self, alias: &str) -> Result<Option<String>, Self::Error>;
    /// Atomically point an alias at a physical collection.
    async fn replace_alias(&self, alias: &str, collection: &str) -> Result<(), Self::Error>;
    /// Delete a physical collection during guarded cleanup.
    async fn remove_collection(&self, collection: &str) -> Result<(), Self::Error>;
}
