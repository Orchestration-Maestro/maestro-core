//! Conversion between backend-neutral projection operations and Qdrant's transport types.

use super::{
    projection_port::{
        CollectionLayout, PointHit, ProjectionFilter, ProjectionPage, ProjectionPoint,
        RetrievalProjectionPort, SparseValues,
    },
    qdrant::{DENSE, Qdrant, QdrantError, SPARSE},
};
use qdrant_client::{
    Payload,
    qdrant::{
        Condition, Distance, Filter, Modifier, NamedVectors, PointId, PointStruct, SparseVector,
        Vector, point_id::PointIdOptions, vectors_config::Config,
    },
};
use std::collections::BTreeMap;

impl RetrievalProjectionPort for Qdrant {
    type Error = QdrantError;

    async fn collection_exists(&self, collection: &str) -> Result<bool, Self::Error> {
        self.exists(collection).await
    }

    async fn create_collection(
        &self,
        collection: &str,
        layout: CollectionLayout,
    ) -> Result<(), Self::Error> {
        if !layout.dense_cosine || !layout.sparse_idf {
            return Err(QdrantError::InvalidAnswer(
                "unsupported retrieval vector layout".to_owned(),
            ));
        }
        self.create(collection, layout.dense_dimensions).await?;
        self.index_search_fields(collection).await
    }

    async fn collection_layout(
        &self,
        collection: &str,
    ) -> Result<Option<CollectionLayout>, Self::Error> {
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
        let sparse_idf = params
            .sparse_vectors_config
            .as_ref()
            .and_then(|config| config.map.get(SPARSE))
            .and_then(|vector| vector.modifier)
            .is_some_and(|modifier| modifier == i32::from(Modifier::Idf));
        Ok(dense.map(|dense| CollectionLayout {
            dense_dimensions: dense.size,
            dense_cosine: dense.distance == i32::from(Distance::Cosine),
            sparse_idf,
        }))
    }

    async fn index_payload_fields(&self, collection: &str) -> Result<(), Self::Error> {
        self.index_search_fields(collection).await
    }

    async fn payload_fields(
        &self,
        collection: &str,
    ) -> Result<BTreeMap<String, String>, Self::Error> {
        Ok(self
            .payload_indexes(collection)
            .await?
            .into_iter()
            .map(|(field, kind)| (field, kind.to_string()))
            .collect())
    }

    async fn upsert(
        &self,
        collection: &str,
        points: Vec<ProjectionPoint>,
    ) -> Result<(), Self::Error> {
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
        Qdrant::upsert(self, collection, points).await
    }

    async fn count(&self, collection: &str) -> Result<u64, Self::Error> {
        Qdrant::count(self, collection).await
    }

    async fn found(&self, collection: &str, ids: &[String]) -> Result<Vec<String>, Self::Error> {
        Qdrant::found(self, collection, ids).await
    }

    async fn payloads(
        &self,
        collection: &str,
        ids: &[String],
    ) -> Result<Vec<PointHit>, Self::Error> {
        self.payload_points(collection, ids)
            .await?
            .into_iter()
            .map(payload_hit)
            .collect()
    }

    async fn query_dense(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error> {
        self.query_dense(collection, vector, limit, to_filter(filter))
            .await?
            .into_iter()
            .map(scored_hit)
            .collect()
    }

    async fn query_sparse(
        &self,
        collection: &str,
        vector: SparseValues,
        limit: usize,
        filter: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error> {
        let sparse = SparseVector {
            indices: vector.indices,
            values: vector.values,
        };
        self.query_sparse(collection, sparse, limit, to_filter(filter))
            .await?
            .into_iter()
            .map(scored_hit)
            .collect()
    }

    async fn scroll(
        &self,
        collection: &str,
        filter: ProjectionFilter,
        cursor: Option<String>,
    ) -> Result<ProjectionPage, Self::Error> {
        let offset = cursor.as_deref().map(PointId::from);
        let page = self
            .scroll_page(collection, to_filter(filter), offset)
            .await?;
        let points = page
            .result
            .into_iter()
            .map(payload_hit)
            .collect::<Result<Vec<_>, _>>()?;
        let next = page
            .next_page_offset
            .and_then(|id| id.point_id_options)
            .and_then(point_id_string);
        Ok(ProjectionPage { points, next })
    }

    async fn alias_target(&self, alias: &str) -> Result<Option<String>, Self::Error> {
        self.alias_collection(alias).await
    }

    async fn replace_alias(&self, alias: &str, collection: &str) -> Result<(), Self::Error> {
        self.point_alias(alias, collection).await
    }

    async fn delete_collection(&self, collection: &str) -> Result<(), Self::Error> {
        self.delete_collection(collection).await
    }
}

fn to_filter(filter: ProjectionFilter) -> Filter {
    fn append(filter: ProjectionFilter, conditions: &mut Vec<Condition>) {
        match filter {
            ProjectionFilter::AnyString { field, values } => {
                conditions.push(Condition::matches(field, values))
            }
            ProjectionFilter::ExactString { field, value } => {
                conditions.push(Condition::matches(field, value))
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

fn payload_hit(point: qdrant_client::qdrant::RetrievedPoint) -> Result<PointHit, QdrantError> {
    let id = point
        .id
        .and_then(|id| id.point_id_options)
        .and_then(point_id_string)
        .ok_or_else(|| QdrantError::InvalidAnswer("payload point has no identifier".to_owned()))?;
    let payload = serde_json::to_value(point.payload)
        .map_err(|error| QdrantError::InvalidAnswer(error.to_string()))?;
    let payload = payload
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();
    Ok(PointHit {
        id,
        score: None,
        payload,
    })
}

fn scored_hit(point: qdrant_client::qdrant::ScoredPoint) -> Result<PointHit, QdrantError> {
    let id = point
        .id
        .and_then(|id| id.point_id_options)
        .and_then(point_id_string)
        .ok_or_else(|| QdrantError::InvalidAnswer("scored point has no identifier".to_owned()))?;
    let payload = serde_json::to_value(point.payload)
        .map_err(|error| QdrantError::InvalidAnswer(error.to_string()))?;
    let payload = payload
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();
    Ok(PointHit {
        id,
        score: Some(f64::from(point.score)),
        payload,
    })
}

fn point_id_string(id: PointIdOptions) -> Option<String> {
    Some(match id {
        PointIdOptions::Uuid(uuid) => uuid,
        PointIdOptions::Num(number) => number.to_string(),
    })
}
