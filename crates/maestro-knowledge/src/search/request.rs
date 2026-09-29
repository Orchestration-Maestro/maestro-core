//! Bounded search requests and retrieval handoffs for T032.

use super::{
    assembly_settings::EvidenceSettings,
    deadline::StageWindow,
    fusion::Route,
    intent::{IntentExpansion, IntentTrigger, QueryExpander},
    rerank::{DEFAULT_DEPTH, Ranked, Reranker},
    routes::{dense::Embedder, error::RouteError, outcome::DroppedIdentifier},
    section_prior::SectionPrior,
    source_class::{SourceClassifier, SourcePrior},
};
use crate::{
    index::{Qdrant, RetrievalProjectionPort},
    query::Understood,
};
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
    /// Optional additive hypothetical-document retrieval.
    pub intent_expansion: IntentExpansion,
    /// Expand always or only after weak original ranking.
    pub intent_trigger: IntentTrigger,
    /// Maximum model expansion time, capped by the search deadline.
    pub intent_deadline_ms: u32,
    /// RRF multiplier for each extra intent route.
    pub intent_weight: f64,
    /// The most candidates the intent votes may add to the rerank beyond
    /// the original top-depth, which the rerank always keeps.
    pub intent_rerank_additions: usize,
    /// Maximum candidates each general retrieval route returns.
    pub routes_limit: usize,
    /// Maximum identifier candidates, additionally bounded by `routes_limit`.
    pub identifier_limit: usize,
    /// Maximum candidates retained through fusion, no greater than 120.
    pub fusion_pool: usize,
    /// Whether dense retrieval runs.
    pub dense_enabled: bool,
    /// Whether lexical retrieval runs.
    pub lexical_enabled: bool,
    /// Whether identifier retrieval runs.
    pub identifier_enabled: bool,
    /// Whether structured retrieval may run for Global questions.
    pub structured_enabled: bool,
    /// Whether the identifier route drops identifiers too common to rank
    /// from both of its legs, and fusion takes no hits from an unavailable
    /// route.
    pub identifier_noise_guard: bool,
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
    /// How long the retrieval routes may run; derived from the request's
    /// budget unless an experiment or a test fixes it.
    pub stage_window: StageWindow,
    /// Soft preference for official sources; it needs a source classifier
    /// in the search context, and ranks as before without one.
    pub source_prior: SourcePrior,
}

impl SearchConfiguration {
    /// Default candidate limit for dense, lexical and intent routes.
    pub const DEFAULT_ROUTES_LIMIT: usize = 100;
    /// Default identifier-route candidate cap.
    pub const DEFAULT_IDENTIFIER_LIMIT: usize = 20;
    /// Default fusion pool and its hard handoff ceiling.
    pub const MAX_FUSION_POOL: usize = 120;
}

impl Default for SearchConfiguration {
    fn default() -> Self {
        #[expect(clippy::expect_used, reason = "60 is the fixed nonzero default")]
        let rrf_k = NonZeroU32::new(60).expect("default RRF K is nonzero");
        Self {
            intent_expansion: IntentExpansion::Off,
            intent_trigger: IntentTrigger::Always,
            intent_deadline_ms: 4000,
            intent_weight: 1.0,
            intent_rerank_additions: 10,
            routes_limit: Self::DEFAULT_ROUTES_LIMIT,
            identifier_limit: Self::DEFAULT_IDENTIFIER_LIMIT,
            fusion_pool: Self::MAX_FUSION_POOL,
            dense_enabled: true,
            lexical_enabled: true,
            identifier_enabled: true,
            structured_enabled: true,
            identifier_noise_guard: false,
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
            stage_window: StageWindow::Derived,
            source_prior: SourcePrior::default(),
        }
    }
}

impl SearchConfiguration {
    /// Returns the configured RRF multiplier for `route`.
    #[must_use]
    pub const fn weight(self, route: Route) -> f64 {
        match route {
            Route::DenseIntent | Route::LexicalIntent => self.intent_weight,
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
            self.intent_weight,
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
    /// How many original top-depth candidates the intent votes put below
    /// the rerank depth, all still reranked; none when no intent voted.
    pub intent_displaced: Option<usize>,
    /// The identifiers the noise guard dropped from the identifier route,
    /// and why.
    pub identifiers_dropped: Vec<DroppedIdentifier>,
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
pub struct SearchContext<'a, P, R = Qdrant> {
    /// The kernel database shared by owned blocking readers.
    pub database: Arc<Database>,
    /// The caller resolved by a trusted transport, never request JSON.
    pub principal: &'a str,
    /// Backend holding the pinned generation's physical collection.
    pub qdrant: &'a R,
    /// The generation's matching embedder, absent when no card is available.
    pub embedder: Option<Embedder<'a, P>>,
    /// Optional explicitly configured expansion model; unused when expansion is off.
    pub intent_expander: Option<Box<dyn QueryExpander + 'a>>,
    /// The configured reranker, when available.
    pub reranker: Option<Reranker<'a, P>>,
    /// The source classifier the source prior reads, when one is configured.
    pub source_classes: Option<Arc<dyn SourceClassifier>>,
}

impl<P, R: RetrievalProjectionPort> fmt::Debug for SearchContext<'_, P, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchContext")
            .field("database", &"Database")
            .field("principal", &self.principal)
            .field("projection", &self.qdrant)
            .field("has_embedder", &self.embedder.is_some())
            .field("has_intent_expander", &self.intent_expander.is_some())
            .field("has_reranker", &self.reranker.is_some())
            .field("source_classes", &self.source_classes)
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
