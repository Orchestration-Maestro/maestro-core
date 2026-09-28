//! Bounded search requests and retrieval handoffs for T032.

use super::{
    assembly_settings::EvidenceSettings,
    fusion::Route,
    rerank::{DEFAULT_DEPTH, Ranked, Reranker},
    routes::{dense::Embedder, error::RouteError},
    section_prior::SectionPrior,
};
use crate::{index::Qdrant, query::Understood};
use maestro_kernel::{
    evidence::{Bundle, Inventory, RequestBudget, RouteStatus},
    generation::Generation,
    retrieval,
    scope::ScopeSet,
    store::Database,
};
use std::{
    collections::BTreeMap,
    error, fmt,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};
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
    /// Search route, fusion and rerank settings; defaults preserve the current pipeline.
    pub configuration: SearchConfiguration,
    /// Assembly policy carried beside the request budget and ranking settings.
    pub evidence: EvidenceSettings,
}

/// Text presented to the reranker; evidence and citation spans are never changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateContext {
    /// Original prepared input, exactly as indexed.
    #[default]
    Chunk,
    /// Whole section or whole-sibling window, falling back to the chunk if oversized.
    BoundedSection {
        /// Maximum expanded text bytes, including the heading path; 1..=1500.
        max_bytes: usize,
    },
}

/// Bounded knobs for one search execution.
#[derive(Clone, Copy, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each route and rerank stage has an independent enable switch"
)]
pub struct SearchConfiguration {
    /// Whether dense retrieval runs.
    pub dense_enabled: bool,
    /// Whether lexical retrieval runs.
    pub lexical_enabled: bool,
    /// Whether identifier retrieval runs.
    pub identifier_enabled: bool,
    /// Whether structured retrieval may run for Global questions.
    pub structured_enabled: bool,
    /// RRF denominator constant.
    pub rrf_k: NonZeroU32,
    /// Dense route's RRF contribution multiplier.
    pub dense_weight: f64,
    /// Lexical route's RRF contribution multiplier.
    pub lexical_weight: f64,
    /// Identifier route's RRF contribution multiplier.
    pub identifier_weight: f64,
    /// Structured route's RRF contribution multiplier.
    pub structured_weight: f64,
    /// Whether to call the reranker.
    pub rerank_enabled: bool,
    /// Fused candidates passed to reranking, capped at 120.
    pub rerank_depth: NonZeroUsize,
    /// The least top reranker score `ask` answers from, when rerank ran;
    /// search results are never filtered by it.
    pub min_rerank_score: Option<f32>,
    /// Fused-position weight in a rank fusion of fused and reranked
    /// positions with the constant `rrf_k`; absent keeps the rerank order.
    pub rerank_blend: Option<f32>,
    /// Maximum positions a fused top-10 candidate may drop after reranking.
    pub rerank_demotion_cap: Option<u16>,
    /// Optional source-section context for reranking.
    pub candidate_context: CandidateContext,
    /// Optional metadata-based soft section penalty.
    pub section_prior: SectionPrior,
}

impl Default for SearchConfiguration {
    fn default() -> Self {
        #[expect(clippy::expect_used, reason = "60 is the fixed nonzero default")]
        let rrf_k = NonZeroU32::new(60).expect("default RRF K is nonzero");
        Self {
            dense_enabled: true,
            lexical_enabled: true,
            identifier_enabled: true,
            structured_enabled: true,
            rrf_k,
            dense_weight: 1.0,
            lexical_weight: 1.0,
            identifier_weight: 1.0,
            structured_weight: 1.0,
            rerank_enabled: true,
            rerank_depth: DEFAULT_DEPTH,
            min_rerank_score: None,
            rerank_blend: None,
            rerank_demotion_cap: None,
            candidate_context: CandidateContext::Chunk,
            section_prior: SectionPrior::Off,
        }
    }
}

impl SearchConfiguration {
    /// Returns the configured RRF multiplier for `route`.
    #[must_use]
    pub const fn weight(self, route: Route) -> f64 {
        match route {
            Route::Dense => self.dense_weight,
            Route::Lexical => self.lexical_weight,
            Route::Identifier => self.identifier_weight,
            Route::Structured => self.structured_weight,
        }
    }

    /// Whether the configured fusion weights are finite and nonnegative.
    #[must_use]
    pub fn weights_are_valid(self) -> bool {
        [
            self.dense_weight,
            self.lexical_weight,
            self.identifier_weight,
            self.structured_weight,
        ]
        .into_iter()
        .all(|weight| weight.is_finite() && weight >= 0.0)
    }
}

/// Rank snapshots retained for evaluation without reconstructing stages from the final bundle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchObservations {
    /// One-based scoped ranks for each route before fusion, including empty failed routes.
    pub route_ranks: BTreeMap<Route, Vec<String>>,
    /// Chunk IDs ordered after reranking or fused-order fallback.
    pub reranked_chunk_ids: Vec<String>,
    /// Chunk IDs grouped by final passage slot, with slots in reading order.
    pub assembled_passages: Vec<Vec<String>>,
    /// Wall time spent loading and validating source context, in microseconds.
    pub candidate_source_load_micros: u64,
    /// Candidate identities that retained the chunk because a whole unit exceeded the cap.
    pub candidate_context_fallbacks: Vec<String>,
}

impl SearchObservations {
    /// Records chunk identities grouped by each final passage slot in reading order.
    pub fn observe_assembly(&mut self, bundle: &Bundle) {
        self.assembled_passages = bundle
            .passages
            .iter()
            .map(|passage| {
                bundle
                    .trace
                    .iter()
                    .find(|trace| trace.n == passage.n)
                    .map_or_else(Vec::new, |trace| trace.chunk_ids.clone())
            })
            .collect();
    }
}

impl<'a> SearchRequest<'a> {
    /// Creates a request with the measured default rerank depth.
    #[must_use]
    pub fn new(
        collection: &'a str,
        text: &'a str,
        version: Option<&'a str>,
        budget: RequestBudget,
    ) -> Self {
        Self {
            collection,
            text,
            version,
            budget,
            configuration: SearchConfiguration::default(),
            evidence: EvidenceSettings::default(),
        }
    }
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
    /// Request-scoped assembly policy; the budget remains in `budget`.
    pub evidence: EvidenceSettings,
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
    /// Per-stage scoped ranks retained for evaluation.
    pub observations: SearchObservations,
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
