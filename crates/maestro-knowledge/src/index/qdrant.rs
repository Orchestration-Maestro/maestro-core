//! Qdrant through its official Rust client, `qdrant-client` 1.19, over gRPC
//! (ADR-0020): only the calls a generation's build, check and alias need.

use qdrant_client::{
    Qdrant as Client, QdrantError as ClientError,
    qdrant::{
        CollectionParams, CountPointsBuilder, CreateAliasBuilder, CreateCollectionBuilder,
        Distance, GetPointsBuilder, Modifier, PointId, PointStruct, SparseVectorParamsBuilder,
        SparseVectorsConfigBuilder, UpsertPointsBuilder, VectorParamsBuilder, VectorsConfigBuilder,
        point_id::PointIdOptions,
    },
};
use std::{error, fmt, time::Duration};

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
    /// `http://127.0.0.1:6334`. It connects at its first request, reads no
    /// environment variable and sends no API key, and gives each request
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

    /// Whether the collection `collection` exists.
    pub(super) async fn exists(&self, collection: &str) -> Result<bool, QdrantError> {
        self.client
            .collection_exists(collection)
            .await
            .map_err(QdrantError::Client)
    }

    /// Creates the collection `collection`: a dense vector [`DENSE`] of
    /// `dimensions` compared by cosine, and a sparse vector [`SPARSE`] whose
    /// weights Qdrant multiplies by each term's IDF.
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
            .sparse_vectors_config(sparse);
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
    pub(super) async fn count(&self, collection: &str) -> Result<u64, QdrantError> {
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
