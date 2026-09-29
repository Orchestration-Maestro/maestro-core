//! Conversion between backend-neutral projection operations and Qdrant's transport types.

use super::{
    projection_port::{
        CollectionLayout, DenseDistance, DenseLayout, PayloadFieldKind, PointHit, ProjectionCursor,
        ProjectionError, ProjectionFilter, ProjectionPage, ProjectionPoint,
        RetrievalProjectionPort, SparseModifier, SparseValues,
    },
    qdrant::{DENSE, Qdrant, QdrantError, SPARSE},
};
use qdrant_client::{
    Payload,
    qdrant::{
        Condition, Distance, Filter, Modifier, NamedVectors, PayloadSchemaType, PointId,
        PointStruct, RetrievedPoint, ScoredPoint, SparseVector, Value, Vector,
        point_id::PointIdOptions, vectors_config::Config,
    },
};
use std::collections::{BTreeMap, HashMap};

impl From<QdrantError> for ProjectionError {
    fn from(error: QdrantError) -> Self {
        Self::with_source(error.to_string(), error)
    }
}

impl From<ProjectionError> for QdrantError {
    fn from(error: ProjectionError) -> Self {
        let (message, source) = error.into_parts();
        source
            .and_then(|source| source.downcast::<QdrantError>().ok())
            .map_or_else(|| Self::InvalidAnswer(message), |error| *error)
    }
}

impl RetrievalProjectionPort for Qdrant {
    async fn collection_exists(&self, collection: &str) -> Result<bool, ProjectionError> {
        self.exists(collection).await.map_err(Into::into)
    }

    async fn create_collection(
        &self,
        collection: &str,
        layout: CollectionLayout,
    ) -> Result<(), ProjectionError> {
        let Some(dense) = layout.dense else {
            return Err(ProjectionError::new("unsupported retrieval vector layout"));
        };
        if dense.distance != DenseDistance::Cosine
            || !layout.sparse_present
            || layout.sparse_modifier != Some(SparseModifier::Idf)
        {
            return Err(ProjectionError::new("unsupported retrieval vector layout"));
        }
        self.create(collection, dense.dimensions)
            .await
            .map_err(Into::into)
    }

    async fn collection_layout(
        &self,
        collection: &str,
    ) -> Result<Option<CollectionLayout>, ProjectionError> {
        if !self.exists(collection).await? {
            return Ok(None);
        }
        let params = self.parameters(collection).await?;
        let dense = params
            .vectors_config
            .as_ref()
            .and_then(|config| config.config.as_ref())
            .and_then(|config| {
                if let Config::ParamsMap(named) = config {
                    named.map.get(DENSE)
                } else {
                    None
                }
            });
        let sparse = params
            .sparse_vectors_config
            .as_ref()
            .and_then(|config| config.map.get(SPARSE));
        let dense = dense.map(|dense| DenseLayout {
            dimensions: dense.size,
            distance: match Distance::try_from(dense.distance) {
                Ok(Distance::Cosine) => DenseDistance::Cosine,
                Ok(Distance::Euclid) => DenseDistance::Euclid,
                Ok(Distance::Dot) => DenseDistance::Dot,
                Ok(Distance::Manhattan) => DenseDistance::Manhattan,
                Ok(Distance::UnknownDistance) => DenseDistance::Unknown,
                Err(_) => DenseDistance::Other,
            },
        });
        let sparse_modifier = sparse
            .and_then(|vector| vector.modifier)
            .and_then(|modifier| Modifier::try_from(modifier).ok())
            .map(|modifier| match modifier {
                Modifier::None => SparseModifier::None,
                Modifier::Idf => SparseModifier::Idf,
            });
        Ok(Some(CollectionLayout {
            dense,
            sparse_present: sparse.is_some(),
            sparse_modifier,
        }))
    }

    async fn index_payload_fields(&self, collection: &str) -> Result<(), ProjectionError> {
        self.index_search_fields(collection)
            .await
            .map_err(Into::into)
    }

    async fn payload_fields(
        &self,
        collection: &str,
    ) -> Result<BTreeMap<String, PayloadFieldKind>, ProjectionError> {
        Ok(self
            .payload_indexes(collection)
            .await?
            .into_iter()
            .map(|(field, kind)| {
                let kind = if kind == i32::from(PayloadSchemaType::Keyword) {
                    PayloadFieldKind::Keyword
                } else {
                    PayloadFieldKind::Other
                };
                (field, kind)
            })
            .collect())
    }

    async fn upsert_points(
        &self,
        collection: &str,
        points: Vec<ProjectionPoint>,
    ) -> Result<(), ProjectionError> {
        let points = points
            .into_iter()
            .map(|point| {
                let vectors = NamedVectors::default()
                    .add_vector(DENSE, Vector::new_dense(point.dense))
                    .add_vector(
                        SPARSE,
                        Vector::new_sparse(point.sparse.indices, point.sparse.values),
                    );
                PointStruct::new(
                    point.id,
                    vectors,
                    Payload::from(point.payload.into_iter().collect::<serde_json::Map<_, _>>()),
                )
            })
            .collect();
        Qdrant::upsert(self, collection, points)
            .await
            .map_err(Into::into)
    }

    async fn count_points(&self, collection: &str) -> Result<u64, ProjectionError> {
        Qdrant::count(self, collection).await.map_err(Into::into)
    }

    async fn point_ids(
        &self,
        collection: &str,
        ids: &[String],
    ) -> Result<Vec<String>, ProjectionError> {
        Qdrant::found(self, collection, ids)
            .await
            .map_err(Into::into)
    }

    async fn payloads(
        &self,
        collection: &str,
        ids: &[String],
    ) -> Result<Vec<PointHit>, ProjectionError> {
        Ok(self
            .payload_points(collection, ids)
            .await?
            .into_iter()
            .map(payload_hit)
            .collect())
    }

    async fn search_dense(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, ProjectionError> {
        Ok(self
            .query_dense(collection, vector, limit, to_filter(filter))
            .await?
            .into_iter()
            .map(scored_hit)
            .collect())
    }

    async fn search_sparse(
        &self,
        collection: &str,
        vector: SparseValues,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, ProjectionError> {
        let sparse = SparseVector {
            indices: vector.indices,
            values: vector.values,
        };
        Ok(self
            .query_sparse(collection, sparse, limit, to_filter(filter))
            .await?
            .into_iter()
            .map(scored_hit)
            .collect())
    }

    async fn scroll(
        &self,
        collection: &str,
        filter: ProjectionFilter,
        cursor: Option<ProjectionCursor>,
    ) -> Result<ProjectionPage, ProjectionError> {
        let offset = cursor.map(|cursor| match cursor {
            ProjectionCursor::Number(number) => PointId::from(number),
            ProjectionCursor::Text(id) => PointId::from(id.as_str()),
        });
        let page = self
            .scroll_page(collection, to_filter(filter), offset)
            .await?;
        let points = page.result.into_iter().map(payload_hit).collect::<Vec<_>>();
        let next = page
            .next_page_offset
            .map(|id| {
                id.point_id_options.map(point_id_cursor).ok_or_else(|| {
                    QdrantError::InvalidAnswer("scroll cursor has no point ID".to_owned())
                })
            })
            .transpose()?;
        Ok(ProjectionPage { points, next })
    }

    async fn alias_target(&self, alias: &str) -> Result<Option<String>, ProjectionError> {
        self.alias_collection(alias).await.map_err(Into::into)
    }

    async fn replace_alias(&self, alias: &str, collection: &str) -> Result<(), ProjectionError> {
        self.point_alias(alias, collection)
            .await
            .map_err(Into::into)
    }
}

/// Converts backend-neutral predicates into Qdrant conditions.
fn to_filter(filter: ProjectionFilter) -> Filter {
    fn append(filter: ProjectionFilter, conditions: &mut Vec<Condition>) {
        match filter {
            ProjectionFilter::AnyString { field, values } => {
                conditions.push(Condition::matches(field, values));
            }
            ProjectionFilter::ExactString { field, value } => {
                conditions.push(Condition::matches(field, value));
            }
            ProjectionFilter::All(filters) => filters
                .into_iter()
                .for_each(|filter| append(filter, conditions)),
        }
    }
    let mut conditions = Vec::new();
    append(filter, &mut conditions);
    Filter::must(conditions)
}

/// Converts Qdrant payload fields and a point ID into a neutral hit.
pub(super) fn point_hit(
    id: Option<PointId>,
    payload: HashMap<String, Value>,
    score: Option<f64>,
) -> PointHit {
    let id = id
        .and_then(|id| id.point_id_options)
        .map_or_else(String::new, point_id_string);
    PointHit {
        id,
        score,
        payload: payload
            .into_iter()
            .map(|(key, value)| (key, value.into_json()))
            .collect(),
    }
}

/// Converts a Qdrant payload-only point to a neutral hit.
fn payload_hit(point: RetrievedPoint) -> PointHit {
    point_hit(point.id, point.payload, None)
}

/// Converts a ranked Qdrant point to a neutral hit.
fn scored_hit(point: ScoredPoint) -> PointHit {
    point_hit(point.id, point.payload, Some(f64::from(point.score)))
}

/// Converts Qdrant's typed point identifier into a backend-neutral cursor.
fn point_id_cursor(id: PointIdOptions) -> ProjectionCursor {
    match id {
        PointIdOptions::Uuid(uuid) => ProjectionCursor::Text(uuid),
        PointIdOptions::Num(number) => ProjectionCursor::Number(number),
    }
}

/// Converts Qdrant's typed point identifier into its stable string form.
fn point_id_string(id: PointIdOptions) -> String {
    match point_id_cursor(id) {
        ProjectionCursor::Number(number) => number.to_string(),
        ProjectionCursor::Text(text) => text,
    }
}
