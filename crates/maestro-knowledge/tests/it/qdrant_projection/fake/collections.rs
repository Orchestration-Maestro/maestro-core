//! The fake's collections: their creation, existence and parameters, and
//! their aliases; the other calls of the service are refused as not served.

use super::state::Fake;
use qdrant_client::qdrant::{
    AliasDescription, ChangeAliases, CollectionClusterInfoRequest, CollectionClusterInfoResponse,
    CollectionConfig, CollectionExists, CollectionExistsRequest, CollectionExistsResponse,
    CollectionInfo, CollectionOperationResponse, CollectionParams, CreateCollection,
    CreateShardKeyRequest, CreateShardKeyResponse, DeleteCollection, DeleteShardKeyRequest,
    DeleteShardKeyResponse, GetCollectionInfoRequest, GetCollectionInfoResponse,
    ListAliasesRequest, ListAliasesResponse, ListCollectionAliasesRequest, ListCollectionsRequest,
    ListCollectionsResponse, ListShardKeysRequest, ListShardKeysResponse, UpdateCollection,
    UpdateCollectionClusterSetupRequest, UpdateCollectionClusterSetupResponse,
    alias_operations::Action, collections_server::Collections,
};
use tonic::{Request, Response, Status};

/// The refusal of a call the fake does not serve.
pub(super) fn unserved(call: &str) -> Status {
    Status::unimplemented(format!("the fake Qdrant does not serve {call}"))
}

#[tonic::async_trait]
impl Collections for Fake {
    async fn get(
        &self,
        request: Request<GetCollectionInfoRequest>,
    ) -> Result<Response<GetCollectionInfoResponse>, Status> {
        self.admit("collection_info")?;
        let name = request.into_inner().collection_name;
        let hollow = self.hollow("collection_info");
        let mut state = self.state();
        let collection = state.collection(&name)?;
        let info = CollectionInfo {
            points_count: Some(collection.points.len() as u64),
            config: Some(CollectionConfig {
                params: Some(collection.params.clone()),
                ..CollectionConfig::default()
            }),
            ..CollectionInfo::default()
        };
        let result = (!hollow).then_some(info);
        Ok(Response::new(GetCollectionInfoResponse {
            result,
            time: 0.0,
        }))
    }

    async fn create(
        &self,
        request: Request<CreateCollection>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        self.admit("create")?;
        let create = request.into_inner();
        let mut state = self.state();
        let name = create.collection_name;
        if state.collections.contains_key(&name) {
            let exists = format!("Wrong input: Collection `{name}` already exists!");
            return Err(Status::invalid_argument(exists));
        }
        let params = CollectionParams {
            vectors_config: create.vectors_config,
            sparse_vectors_config: create.sparse_vectors_config,
            ..CollectionParams::default()
        };
        let collection = super::state::Collection {
            params,
            ..super::state::Collection::default()
        };
        state.collections.insert(name, collection);
        Ok(Response::new(CollectionOperationResponse {
            result: true,
            time: 0.0,
        }))
    }

    async fn update_aliases(
        &self,
        request: Request<ChangeAliases>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        self.admit("update_aliases")?;
        let mut state = self.state();
        for operation in request.into_inner().actions {
            match operation.action {
                Some(Action::CreateAlias(create)) => {
                    state.collection(&create.collection_name)?;
                    state
                        .aliases
                        .insert(create.alias_name, create.collection_name);
                }
                Some(Action::DeleteAlias(delete)) => drop(state.aliases.remove(&delete.alias_name)),
                Some(Action::RenameAlias(_)) | None => return Err(unserved("rename_alias")),
            }
        }
        Ok(Response::new(CollectionOperationResponse {
            result: true,
            time: 0.0,
        }))
    }

    async fn list_aliases(
        &self,
        _request: Request<ListAliasesRequest>,
    ) -> Result<Response<ListAliasesResponse>, Status> {
        let aliases = self
            .state()
            .aliases
            .iter()
            .map(|(alias, collection)| AliasDescription {
                alias_name: alias.clone(),
                collection_name: collection.clone(),
            })
            .collect();
        Ok(Response::new(ListAliasesResponse { aliases, time: 0.0 }))
    }

    async fn collection_exists(
        &self,
        request: Request<CollectionExistsRequest>,
    ) -> Result<Response<CollectionExistsResponse>, Status> {
        self.admit("collection_exists")?;
        let name = request.into_inner().collection_name;
        let exists = self.state().collections.contains_key(&name);
        let result = Some(CollectionExists { exists });
        Ok(Response::new(CollectionExistsResponse {
            result,
            time: 0.0,
        }))
    }

    async fn list(
        &self,
        _request: Request<ListCollectionsRequest>,
    ) -> Result<Response<ListCollectionsResponse>, Status> {
        Err(unserved("list"))
    }

    async fn update(
        &self,
        _request: Request<UpdateCollection>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        Err(unserved("update"))
    }

    async fn delete(
        &self,
        _request: Request<DeleteCollection>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        Err(unserved("delete"))
    }

    async fn list_collection_aliases(
        &self,
        _request: Request<ListCollectionAliasesRequest>,
    ) -> Result<Response<ListAliasesResponse>, Status> {
        Err(unserved("list_collection_aliases"))
    }

    async fn collection_cluster_info(
        &self,
        _request: Request<CollectionClusterInfoRequest>,
    ) -> Result<Response<CollectionClusterInfoResponse>, Status> {
        Err(unserved("collection_cluster_info"))
    }

    async fn update_collection_cluster_setup(
        &self,
        _request: Request<UpdateCollectionClusterSetupRequest>,
    ) -> Result<Response<UpdateCollectionClusterSetupResponse>, Status> {
        Err(unserved("update_collection_cluster_setup"))
    }

    async fn create_shard_key(
        &self,
        _request: Request<CreateShardKeyRequest>,
    ) -> Result<Response<CreateShardKeyResponse>, Status> {
        Err(unserved("create_shard_key"))
    }

    async fn delete_shard_key(
        &self,
        _request: Request<DeleteShardKeyRequest>,
    ) -> Result<Response<DeleteShardKeyResponse>, Status> {
        Err(unserved("delete_shard_key"))
    }

    async fn list_shard_keys(
        &self,
        _request: Request<ListShardKeysRequest>,
    ) -> Result<Response<ListShardKeysResponse>, Status> {
        Err(unserved("list_shard_keys"))
    }
}
