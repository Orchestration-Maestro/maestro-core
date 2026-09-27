//! Qdrant through its official Rust client, `qdrant-client` 1.19, over gRPC
//! (ADR-0020): only the calls a generation's build, check and alias need.

use qdrant_client::{
    Qdrant as Client, QdrantError as ClientError,
    qdrant::{
        CollectionParams, CountPointsBuilder, CreateAliasBuilder, CreateCollectionBuilder,
        CreateFieldIndexCollectionBuilder, Distance, FieldType, Filter, GetPointsBuilder,
        HnswConfigDiffBuilder, Modifier, PointId, PointStruct, QueryPointsBuilder, RetrievedPoint,
        ScoredPoint, ScrollPointsBuilder, ScrollResponse, SparseVector, SparseVectorParamsBuilder,
        SparseVectorsConfigBuilder, UpsertPointsBuilder, VectorInput, VectorParamsBuilder,
        VectorsConfigBuilder, point_id::PointIdOptions,
    },
};
use std::{collections::HashMap, error, fmt, time::Duration};

/// The name of the dense vector of every generation's collection.
pub(super) const DENSE: &str = "dense";

/// The name of the sparse vector of every generation's collection.
pub(super) const SPARSE: &str = "bm25";

/// How long one request may take: an upsert returns once its points are
/// applied.
const DEADLINE: Duration = Duration::from_secs(60);

/// A client of a Qdrant server's gRPC API.
pub struct Qdrant {
    /// The official client, which keeps its connections open.
    client: Client,
}

impl fmt::Debug for Qdrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Qdrant")
            .field("url", &self.client.config.uri)
            .finish_non_exhaustive()
    }
}

impl Qdrant {
    /// A client of the Qdrant whose gRPC API answers at `url`, such as
    /// `http://127.0.0.1:16634` for the scratch server. It connects at its
    /// first request, reads no environment variable and sends no API key,
    /// and gives each request
    /// 60 s.
    ///
    /// # Errors
    ///
    /// [`QdrantError::Client`] when `url` is not a URI.
    pub fn new(url: &str) -> Result<Self, QdrantError> {
        let client = Client::from_url(url)
            .timeout(DEADLINE)
            .skip_compatibility_check()
            .build()
            .map_err(QdrantError::Client)?;
        Ok(Self { client })
    }

    /// Qdrant's version, returned by its gRPC health check.
    ///
    /// # Errors
    ///
    /// [`QdrantError::Client`] when Qdrant refuses or cannot answer.
    pub async fn version(&self) -> Result<String, QdrantError> {
        self.client
            .health_check()
            .await
            .map(|answer| answer.version)
            .map_err(QdrantError::Client)
    }

    /// Whether the collection `collection` exists.
    ///
    /// # Errors
    ///
    /// [`QdrantError::Client`] when Qdrant refuses or cannot answer.
    pub async fn exists(&self, collection: &str) -> Result<bool, QdrantError> {
        self.client
            .collection_exists(collection)
            .await
            .map_err(QdrantError::Client)
    }

    /// Creates the collection `collection`: a dense vector [`DENSE`] of
    /// `dimensions` compared by cosine, a sparse vector [`SPARSE`] whose
    /// weights Qdrant multiplies by each term's IDF, and HNSW with
    /// `ef_construct = 200`.
    pub(super) async fn create(
        &self,
        collection: &str,
        dimensions: u64,
    ) -> Result<(), QdrantError> {
        let mut dense = VectorsConfigBuilder::default();
        dense.add_named_vector_params(
            DENSE,
            VectorParamsBuilder::new(dimensions, Distance::Cosine),
        );
        let mut sparse = SparseVectorsConfigBuilder::default();
        sparse.add_named_vector_params(
            SPARSE,
            SparseVectorParamsBuilder::default().modifier(Modifier::Idf),
        );
        let create = CreateCollectionBuilder::new(collection)
            .vectors_config(dense)
            .sparse_vectors_config(sparse)
            .hnsw_config(HnswConfigDiffBuilder::default().ef_construct(200));
        self.client
            .create_collection(create)
            .await
            .map(drop)
            .map_err(QdrantError::Client)
    }

    /// The parameters of the collection `collection` as Qdrant reports them,
    /// its vectors and its sparse vectors among them.
    pub(super) async fn parameters(
        &self,
        collection: &str,
    ) -> Result<CollectionParams, QdrantError> {
        let info = self
            .client
            .collection_info(collection)
            .await
            .map_err(QdrantError::Client)?;
        info.result
            .and_then(|info| info.config)
            .and_then(|config| config.params)
            .ok_or_else(|| QdrantError::InvalidAnswer(format!("no parameters of {collection}")))
    }

    /// Creates the keyword indexes required by scoped, exact, and versioned search.
    ///
    /// # Errors
    ///
    /// [`QdrantError::Client`] when Qdrant refuses an index request.
    pub(crate) async fn index_search_fields(&self, collection: &str) -> Result<(), QdrantError> {
        for field in ["scope_tags", "identifiers", "identifier_profile", "version"] {
            self.client
                .create_field_index(
                    CreateFieldIndexCollectionBuilder::new(collection, field, FieldType::Keyword)
                        .wait(true),
                )
                .await
                .map(drop)
                .map_err(QdrantError::Client)?;
        }
        Ok(())
    }

    /// The keyword index types Qdrant reports for this collection.
    pub(crate) async fn payload_indexes(
        &self,
        collection: &str,
    ) -> Result<HashMap<String, i32>, QdrantError> {
        let info = self
            .client
            .collection_info(collection)
            .await
            .map_err(QdrantError::Client)?
            .result
            .ok_or_else(|| {
                QdrantError::InvalidAnswer(format!("no information for {collection}"))
            })?;
        Ok(info
            .payload_schema
            .into_iter()
            .map(|(field, schema)| (field, schema.data_type))
            .collect())
    }

    /// Reads the payload of points named by their physical point IDs.
    pub(crate) async fn payload_points(
        &self,
        collection: &str,
        ids: &[String],
    ) -> Result<Vec<RetrievedPoint>, QdrantError> {
        let ids: Vec<PointId> = ids.iter().map(|id| PointId::from(id.as_str())).collect();
        self.client
            .get_points(
                GetPointsBuilder::new(collection, ids)
                    .with_payload(true)
                    .with_vectors(false),
            )
            .await
            .map(|response| response.result)
            .map_err(QdrantError::Client)
    }

    /// Scrolls one filtered page of payloads from the physical collection.
    pub(crate) async fn scroll_page(
        &self,
        collection: &str,
        filter: Filter,
        offset: Option<PointId>,
    ) -> Result<ScrollResponse, QdrantError> {
        let mut request = ScrollPointsBuilder::new(collection)
            .limit(64)
            .filter(filter)
            .with_payload(true)
            .with_vectors(false);
        if let Some(offset) = offset {
            request = request.offset(offset);
        }
        self.client
            .scroll(request)
            .await
            .map_err(QdrantError::Client)
    }

    /// Queries the named dense vector `DENSE`, applying `filter` in Qdrant
    /// before the top `limit` points are ranked.
    /// The search stays approximate, Qdrant's default; exact search is never asked.
    ///
    /// # Errors
    ///
    /// [`QdrantError::Client`] when Qdrant refuses the query.
    pub(crate) async fn query_dense(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
        filter: Filter,
    ) -> Result<Vec<ScoredPoint>, QdrantError> {
        self.client
            .query(
                QueryPointsBuilder::new(collection)
                    .using(DENSE)
                    .query(vector)
                    .limit(u64::try_from(limit).unwrap_or(u64::MAX))
                    .filter(filter)
                    .with_payload(true),
            )
            .await
            .map(|response| response.result)
            .map_err(QdrantError::Client)
    }

    /// Queries the named sparse vector `SPARSE`, applying `filter` in Qdrant
    /// before the top `limit` points are ranked.
    /// The search stays approximate, Qdrant's default; exact search is never asked.
    ///
    /// # Errors
    ///
    /// [`QdrantError::Client`] when Qdrant refuses the query.
    pub(crate) async fn query_sparse(
        &self,
        collection: &str,
        vector: SparseVector,
        limit: usize,
        filter: Filter,
    ) -> Result<Vec<ScoredPoint>, QdrantError> {
        self.client
            .query(
                QueryPointsBuilder::new(collection)
                    .using(SPARSE)
                    .query(VectorInput::new_sparse(vector.indices, vector.values))
                    .limit(u64::try_from(limit).unwrap_or(u64::MAX))
                    .filter(filter)
                    .with_payload(true),
            )
            .await
            .map(|response| response.result)
            .map_err(QdrantError::Client)
    }

    /// Writes `points` into the collection `collection`, replacing any of the
    /// same IDs, and returns once they are applied.
    pub(super) async fn upsert(
        &self,
        collection: &str,
        points: Vec<PointStruct>,
    ) -> Result<(), QdrantError> {
        self.client
            .upsert_points(UpsertPointsBuilder::new(collection, points).wait(true))
            .await
            .map(drop)
            .map_err(QdrantError::Client)
    }

    /// How many points the collection `collection` holds, counted exactly.
    ///
    /// # Errors
    ///
    /// [`QdrantError`] when Qdrant refuses or cannot answer.
    pub async fn count(&self, collection: &str) -> Result<u64, QdrantError> {
        let counted = self
            .client
            .count(CountPointsBuilder::new(collection).exact(true))
            .await
            .map_err(QdrantError::Client)?;
        counted
            .result
            .map(|result| result.count)
            .ok_or_else(|| QdrantError::InvalidAnswer(format!("no count of {collection}")))
    }

    /// The IDs of `ids` the collection `collection` holds a point of.
    pub(super) async fn found(
        &self,
        collection: &str,
        ids: &[String],
    ) -> Result<Vec<String>, QdrantError> {
        let ids: Vec<PointId> = ids.iter().map(|id| PointId::from(id.as_str())).collect();
        let request = GetPointsBuilder::new(collection, ids)
            .with_payload(false)
            .with_vectors(false);
        let found = self
            .client
            .get_points(request)
            .await
            .map_err(QdrantError::Client)?;
        Ok(found
            .result
            .into_iter()
            .filter_map(|point| point.id?.point_id_options)
            .map(|id| match id {
                PointIdOptions::Uuid(uuid) => uuid,
                PointIdOptions::Num(number) => number.to_string(),
            })
            .collect())
    }

    /// The collection the alias `alias` points at, or none when Qdrant has
    /// no such alias.
    ///
    /// # Errors
    ///
    /// [`QdrantError`] when Qdrant refuses or cannot answer.
    pub async fn alias_collection(&self, alias: &str) -> Result<Option<String>, QdrantError> {
        let aliases = self
            .client
            .list_aliases()
            .await
            .map_err(QdrantError::Client)?;
        Ok(aliases
            .aliases
            .into_iter()
            .find(|entry| entry.alias_name == alias)
            .map(|entry| entry.collection_name))
    }

    /// Points the alias `alias` at the collection `collection`, in one
    /// action: Qdrant's `create_alias` replaces the alias it names, while a
    /// list of actions is not applied atomically (a `delete_alias` stays done
    /// when a later action fails), so readers never find the alias missing.
    pub(super) async fn point_alias(
        &self,
        alias: &str,
        collection: &str,
    ) -> Result<(), QdrantError> {
        self.client
            .create_alias(CreateAliasBuilder::new(collection, alias))
            .await
            .map(drop)
            .map_err(QdrantError::Client)
    }
}

/// Why Qdrant did not do what it was asked.
#[derive(Debug)]
pub enum QdrantError {
    /// The client refused, or Qdrant answered with an error: an unreachable
    /// server among them.
    Client(ClientError),
    /// The answer lacks what the request asked for.
    InvalidAnswer(String),
}

impl fmt::Display for QdrantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(error) => write!(formatter, "Qdrant refused: {error}"),
            Self::InvalidAnswer(reason) => {
                write!(
                    formatter,
                    "Qdrant's answer is not what was asked for: {reason}"
                )
            }
        }
    }
}

impl error::Error for QdrantError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Client(error) => Some(error),
            Self::InvalidAnswer(_) => None,
        }
    }
}
