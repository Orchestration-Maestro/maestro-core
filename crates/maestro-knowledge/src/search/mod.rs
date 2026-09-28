//! Independent retrieval routes, fusion and reranking for knowledge search.

mod admission;
mod assembly_settings;
mod candidate_enrichment;
mod candidates;
mod deadline;
pub use deadline::{DEADLINE_EXCEEDED, DISABLED_BY_CONFIGURATION, StageWindow};
/// Public evidence assembly for ranked T029c search handoffs.
pub mod evidence;
mod filter;
mod fusion;
mod hyde;
pub use hyde::{HYDE_PROMPT_VERSION, HydeCardError, HydeExpander};
mod intent;
mod intent_guard;
mod intent_routes;
mod intent_words;
mod inventory_query;
pub use intent::{
    Expansion, ExpansionFailure, ExpansionFuture, IntentExpansion, IntentTrigger, QueryExpander,
};
mod orchestrate;
mod pin;
mod query;
mod rank_policy;
mod rank_stage;
mod request;
mod rerank;
mod section_prior;
pub use section_prior::{SectionClassSet, SectionPrior};
mod route_execution;
mod route_search;
pub mod routes;
#[cfg(test)]
mod tests;

pub use fusion::{Fused, Hit, Route, RouteList, fuse, fuse_weighted};
pub use orchestrate::search;
pub use pin::pin;
pub use query::Query;
pub use request::{
    CandidateContext, EvidenceInput, SearchConfiguration, SearchContext, SearchError,
    SearchObservations, SearchRequest,
};
pub use rerank::{
    Candidate, NO_FUSED_CANDIDATES, Ranked, Reranked, Reranker, best_rerank_score, rerank,
    top_fused_score, top_rerank_score,
};
pub use routes::outcome::{RouteOutcome, StructuredOutcome};
