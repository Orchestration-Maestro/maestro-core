//! The fake's points: written with their vectors checked as Qdrant checks
//! them, deleted, counted, found by ID and scrolled; the other calls of the
//! service are refused as not served.

use super::{
    collections::unserved,
    state::{Fake, key},
};
use qdrant_client::qdrant::{
    ClearPayloadPoints, CollectionParams, CountPoints, CountResponse, CountResult,
    CreateFieldIndexCollection, CreateVectorNameRequest, DeleteFieldIndexCollection,
    DeletePayloadPoints, DeletePointVectors, DeletePoints, DeleteVectorNameRequest, DenseVector,
    DiscoverBatchPoints, DiscoverBatchResponse, DiscoverPoints, DiscoverResponse, Distance,
    FacetCounts, FacetResponse, GetPoints, GetResponse, NamedVectorsOutput, PointStruct,
    PointsOperationResponse, QueryBatchPoints, QueryBatchResponse, QueryGroupsResponse,
    QueryPointGroups, QueryPoints, QueryResponse, RecommendBatchPoints, RecommendBatchResponse,
    RecommendGroupsResponse, RecommendPointGroups, RecommendPoints, RecommendResponse,
    RetrievedPoint, ScrollPoints, ScrollResponse, SearchBatchPoints, SearchBatchResponse,
    SearchGroupsResponse, SearchMatrixOffsetsResponse, SearchMatrixPairsResponse,
    SearchMatrixPoints, SearchPointGroups, SearchPoints, SearchResponse, SetPayloadPoints,
    UpdateBatchPoints, UpdateBatchResponse, UpdatePointVectors, UpdateResult, UpdateStatus,
    UpsertPoints, VectorOutput, VectorsOutput, points_selector::PointsSelectorOneOf,
    points_server::Points, vector, vector_output, vectors::VectorsOptions, vectors_config::Config,
    vectors_output, with_payload_selector, with_vectors_selector,
};
use std::collections::HashMap;
use tonic::{Request, Response, Status};

/// The answer to a write that is done.
fn completed() -> PointsOperationResponse {
    let result = UpdateResult {
        operation_id: Some(0),
        status: UpdateStatus::Completed.into(),
    };
    PointsOperationResponse {
        result: Some(result),
        time: 0.0,
        usage: None,
    }
}

/// `point` as a collection of `params` keeps it: each dense vector checked
/// against its size and normalized when compared by cosine, as Qdrant does.
fn stored(point: PointStruct, params: &CollectionParams) -> Result<RetrievedPoint, Status> {
    let Some(VectorsOptions::Vectors(named)) =
        point.vectors.and_then(|vectors| vectors.vectors_options)
    else {
        return Err(Status::invalid_argument(
            "the fake keeps named vectors only",
        ));
    };
    let sizes = match params
        .vectors_config
        .as_ref()
        .and_then(|config| config.config.as_ref())
    {
        Some(Config::ParamsMap(map)) => map.map.clone(),
        _ => HashMap::new(),
    };
    let mut vectors = HashMap::new();
    for (name, vector) in named.vectors {
        let output = match (vector.vector, sizes.get(&name)) {
            (Some(vector::Vector::Dense(dense)), Some(size)) => {
                if dense.data.len() as u64 != size.size {
                    return Err(Status::invalid_argument(format!(
                        "Wrong input: Vector dimension error: expected dim: {}, got {}",
                        size.size,
                        dense.data.len()
                    )));
                }
                let data = if size.distance == i32::from(Distance::Cosine) {
                    let norm = dense
                        .data
                        .iter()
                        .map(|value| value * value)
                        .sum::<f32>()
                        .sqrt();
                    dense.data.iter().map(|value| value / norm).collect()
                } else {
                    dense.data
                };
                vector_output::Vector::Dense(DenseVector { data })
            }
            (Some(vector::Vector::Sparse(sparse)), None) => vector_output::Vector::Sparse(sparse),
            _ => {
                return Err(Status::invalid_argument(format!(
                    "Wrong input: vector {name}"
                )));
            }
        };
        let output = VectorOutput {
            vector: Some(output),
            ..VectorOutput::default()
        };
        vectors.insert(name, output);
    }
    let vectors = NamedVectorsOutput { vectors };
    Ok(RetrievedPoint {
        id: point.id,
        payload: point.payload,
        vectors: Some(VectorsOutput {
            vectors_options: Some(vectors_output::VectorsOptions::Vectors(vectors)),
        }),
        shard_key: None,
        order_value: None,
    })
}

/// `point` with its payload when `payload` asks for it, and its vectors when
/// `vectors` does.
fn shown(
    point: &RetrievedPoint,
    payload: Option<&with_payload_selector::SelectorOptions>,
    vectors: Option<&with_vectors_selector::SelectorOptions>,
) -> RetrievedPoint {
    let payload = matches!(
        payload,
        Some(with_payload_selector::SelectorOptions::Enable(true))
    );
    let vectors = matches!(
        vectors,
        Some(with_vectors_selector::SelectorOptions::Enable(true))
    );
    RetrievedPoint {
        id: point.id.clone(),
        payload: if payload {
            point.payload.clone()
        } else {
            HashMap::new()
        },
        vectors: if vectors { point.vectors.clone() } else { None },
        shard_key: None,
        order_value: None,
    }
}

#[tonic::async_trait]
impl Points for Fake {
    async fn upsert(
        &self,
        request: Request<UpsertPoints>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        self.admit("upsert")?;
        let upsert = request.into_inner();
        let mut state = self.state();
        let collection = state.collection(&upsert.collection_name)?;
        let points = upsert
            .points
            .into_iter()
            .map(|point| stored(point, &collection.params))
            .collect::<Result<Vec<_>, _>>()?;
        for point in points {
            let id = key(point.id.as_ref().unwrap());
            collection.points.insert(id, point);
        }
        Ok(Response::new(completed()))
    }

    async fn delete(
        &self,
        request: Request<DeletePoints>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        let delete = request.into_inner();
        let mut state = self.state();
        let collection = state.collection(&delete.collection_name)?;
        let selector = delete
            .points
            .and_then(|points| points.points_selector_one_of);
        let Some(PointsSelectorOneOf::Points(ids)) = selector else {
            return Err(unserved("delete by filter"));
        };
        for id in &ids.ids {
            collection.points.remove(&key(id));
        }
        Ok(Response::new(completed()))
    }

    async fn get(&self, request: Request<GetPoints>) -> Result<Response<GetResponse>, Status> {
        self.admit("get")?;
        let get = request.into_inner();
        let mut state = self.state();
        let collection = state.collection(&get.collection_name)?;
        let payload = get
            .with_payload
            .and_then(|selector| selector.selector_options);
        let vectors = get
            .with_vectors
            .and_then(|selector| selector.selector_options);
        let result = get
            .ids
            .iter()
            .filter_map(|id| collection.points.get(&key(id)))
            .map(|point| shown(point, payload.as_ref(), vectors.as_ref()))
            .collect();
        Ok(Response::new(GetResponse {
            result,
            time: 0.0,
            usage: None,
        }))
    }

    async fn scroll(
        &self,
        request: Request<ScrollPoints>,
    ) -> Result<Response<ScrollResponse>, Status> {
        let scroll = request.into_inner();
        let mut state = self.state();
        let collection = state.collection(&scroll.collection_name)?;
        let payload = scroll
            .with_payload
            .and_then(|selector| selector.selector_options);
        let vectors = scroll
            .with_vectors
            .and_then(|selector| selector.selector_options);
        let result = collection
            .points
            .values()
            .map(|point| shown(point, payload.as_ref(), vectors.as_ref()))
            .collect();
        Ok(Response::new(ScrollResponse {
            next_page_offset: None,
            result,
            time: 0.0,
            usage: None,
        }))
    }

    async fn count(
        &self,
        request: Request<CountPoints>,
    ) -> Result<Response<CountResponse>, Status> {
        self.admit("count")?;
        let name = request.into_inner().collection_name;
        let hollow = self.hollow("count");
        let mut state = self.state();
        let count = state.collection(&name)?.points.len() as u64;
        let result = (!hollow).then_some(CountResult { count });
        Ok(Response::new(CountResponse {
            result,
            time: 0.0,
            usage: None,
        }))
    }

    async fn update_vectors(
        &self,
        _request: Request<UpdatePointVectors>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("update_vectors"))
    }

    async fn delete_vectors(
        &self,
        _request: Request<DeletePointVectors>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("delete_vectors"))
    }

    async fn set_payload(
        &self,
        _request: Request<SetPayloadPoints>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("set_payload"))
    }

    async fn overwrite_payload(
        &self,
        _request: Request<SetPayloadPoints>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("overwrite_payload"))
    }

    async fn delete_payload(
        &self,
        _request: Request<DeletePayloadPoints>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("delete_payload"))
    }

    async fn clear_payload(
        &self,
        _request: Request<ClearPayloadPoints>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("clear_payload"))
    }

    async fn create_field_index(
        &self,
        _request: Request<CreateFieldIndexCollection>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("create_field_index"))
    }

    async fn delete_field_index(
        &self,
        _request: Request<DeleteFieldIndexCollection>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("delete_field_index"))
    }

    async fn create_vector_name(
        &self,
        _request: Request<CreateVectorNameRequest>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("create_vector_name"))
    }

    async fn delete_vector_name(
        &self,
        _request: Request<DeleteVectorNameRequest>,
    ) -> Result<Response<PointsOperationResponse>, Status> {
        Err(unserved("delete_vector_name"))
    }

    async fn search(
        &self,
        _request: Request<SearchPoints>,
    ) -> Result<Response<SearchResponse>, Status> {
        Err(unserved("search"))
    }

    async fn search_batch(
        &self,
        _request: Request<SearchBatchPoints>,
    ) -> Result<Response<SearchBatchResponse>, Status> {
        Err(unserved("search_batch"))
    }

    async fn search_groups(
        &self,
        _request: Request<SearchPointGroups>,
    ) -> Result<Response<SearchGroupsResponse>, Status> {
        Err(unserved("search_groups"))
    }

    async fn recommend(
        &self,
        _request: Request<RecommendPoints>,
    ) -> Result<Response<RecommendResponse>, Status> {
        Err(unserved("recommend"))
    }

    async fn recommend_batch(
        &self,
        _request: Request<RecommendBatchPoints>,
    ) -> Result<Response<RecommendBatchResponse>, Status> {
        Err(unserved("recommend_batch"))
    }

    async fn recommend_groups(
        &self,
        _request: Request<RecommendPointGroups>,
    ) -> Result<Response<RecommendGroupsResponse>, Status> {
        Err(unserved("recommend_groups"))
    }

    async fn discover(
        &self,
        _request: Request<DiscoverPoints>,
    ) -> Result<Response<DiscoverResponse>, Status> {
        Err(unserved("discover"))
    }

    async fn discover_batch(
        &self,
        _request: Request<DiscoverBatchPoints>,
    ) -> Result<Response<DiscoverBatchResponse>, Status> {
        Err(unserved("discover_batch"))
    }

    async fn update_batch(
        &self,
        _request: Request<UpdateBatchPoints>,
    ) -> Result<Response<UpdateBatchResponse>, Status> {
        Err(unserved("update_batch"))
    }

    async fn query(
        &self,
        _request: Request<QueryPoints>,
    ) -> Result<Response<QueryResponse>, Status> {
        Err(unserved("query"))
    }

    async fn query_batch(
        &self,
        _request: Request<QueryBatchPoints>,
    ) -> Result<Response<QueryBatchResponse>, Status> {
        Err(unserved("query_batch"))
    }

    async fn query_groups(
        &self,
        _request: Request<QueryPointGroups>,
    ) -> Result<Response<QueryGroupsResponse>, Status> {
        Err(unserved("query_groups"))
    }

    async fn facet(
        &self,
        _request: Request<FacetCounts>,
    ) -> Result<Response<FacetResponse>, Status> {
        Err(unserved("facet"))
    }

    async fn search_matrix_pairs(
        &self,
        _request: Request<SearchMatrixPoints>,
    ) -> Result<Response<SearchMatrixPairsResponse>, Status> {
        Err(unserved("search_matrix_pairs"))
    }

    async fn search_matrix_offsets(
        &self,
        _request: Request<SearchMatrixPoints>,
    ) -> Result<Response<SearchMatrixOffsetsResponse>, Status> {
        Err(unserved("search_matrix_offsets"))
    }
}
