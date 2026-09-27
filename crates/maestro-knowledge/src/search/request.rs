//! Bounded search requests and retrieval handoffs for T032.

use super::{
    rerank::{Ranked, Reranker},
    routes::{dense::Embedder, error::RouteError},
};
use crate::{index::Qdrant, query::Understood};
use maestro_kernel::{
    evidence::{Inventory, RequestBudget, RouteStatus},
    generation::Generation,
    retrieval,
    scope::ScopeSet,
    store::Database,
};
use std::{collections::BTreeMap, error, fmt, num::NonZeroUsize, sync::Arc};
use tokio::time::Instant;

/// A bounded search request, before resolving caller permissions.
#[derive(Debug)]
pub struct SearchRequest<'a> {
    /// The collection selected by trusted caller input.
    pub collection: &'a str,
    /// The original user question.
    pub text: &'a str,
    /// An exact caller-selected version, when present.
    pub version: Option<&'a str>,
    /// Accepted passage, token and deadline bounds.
    pub budget: RequestBudget,
    /// The number of fused candidates passed to T031, at most 120.
    pub rerank_depth: NonZeroUsize,
}

/// Trusted dependencies and caller identity for one search.
pub struct SearchContext<'a, P> {
    /// The kernel database shared by owned blocking readers.
    pub database: Arc<Database>,
    /// The caller resolved by a trusted transport, never request JSON.
    pub principal: &'a str,
    /// Qdrant containing the pinned generation's physical collection.
    pub qdrant: &'a Qdrant,
    /// The generation's matching embedder, absent when no card is available.
    pub embedder: Option<Embedder<'a, P>>,
    /// The configured reranker, when available.
    pub reranker: Option<Reranker<'a, P>>,
}

impl<P> fmt::Debug for SearchContext<'_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchContext")
            .field("database", &"Database")
            .field("principal", &self.principal)
            .field("qdrant", &self.qdrant)
            .field("has_embedder", &self.embedder.is_some())
            .field("has_reranker", &self.reranker.is_some())
            .finish()
    }
}

/// Why search could not safely complete its handoff.
#[derive(Debug)]
pub enum SearchError {
    /// The request violates the bounded search contract.
    InvalidRequest {
        /// The specific rejected bound or field.
        reason: String,
    },
    /// Scoped collection admission or generation pinning failed.
    Admission(RouteError),
    /// A controlled kernel operation failed.
    Kernel(retrieval::Error),
    /// Initial scoped admission exceeded the accepted deadline.
    AdmissionTimedOut,
    /// A permission recheck exceeded its absolute cutoff.
    PermissionCheckTimedOut,
    /// A fused candidate could not be loaded from the pinned kernel generation.
    EvidenceLoad {
        /// A safe reason that does not name data or local paths.
        reason: String,
    },
    /// The caller's permissions changed during retrieval.
    PermissionsChanged,
    /// A blocking worker panicked or could not be joined.
    WorkerFailed,
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest { reason } => {
                write!(formatter, "invalid search request: {reason}")
            }
            Self::Admission(_) => formatter.write_str("search admission failed"),
            Self::Kernel(_) => formatter.write_str("search kernel operation failed"),
            Self::AdmissionTimedOut => formatter.write_str("search admission timed out"),
            Self::PermissionCheckTimedOut => {
                formatter.write_str("search permission check timed out")
            }
            Self::EvidenceLoad { reason } => {
                write!(formatter, "candidate evidence load failed: {reason}")
            }
            Self::PermissionsChanged => formatter.write_str("permissions changed during search"),
            Self::WorkerFailed => formatter.write_str("search worker failed"),
        }
    }
}

impl error::Error for SearchError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Kernel(error) => Some(error),
            Self::InvalidRequest { .. }
            | Self::AdmissionTimedOut
            | Self::PermissionCheckTimedOut
            | Self::EvidenceLoad { .. }
            | Self::PermissionsChanged
            | Self::WorkerFailed => None,
        }
    }
}

/// Retrieval results handed to T032 without assembling an evidence bundle.
#[derive(Debug, Clone)]
pub struct EvidenceInput {
    /// The single generation pinned at admission.
    pub generation: Generation,
    /// The original question.
    pub query: String,
    /// The bounded local interpretation of that question.
    pub understood: Understood,
    /// The exact effective version filter.
    pub version: Option<String>,
    /// The trusted caller whose permissions admitted this search.
    pub principal: String,
    /// The immutable scope snapshot accepted at admission.
    pub scopes: Arc<ScopeSet>,
    /// The final reranked candidates, with fusion data preserved.
    pub ranked: Vec<Ranked>,
    /// The status of each route that participated in the search.
    pub routes: BTreeMap<String, RouteStatus>,
    /// The complete exact inventory, if structured routing succeeded.
    pub inventory: Option<Inventory>,
    /// The accepted request limits echoed to evidence.
    pub budget: RequestBudget,
    /// The absolute deadline shared with T032.
    pub deadline: Instant,
    /// Gaps known before evidence expansion.
    pub known_gaps: Vec<String>,
}
