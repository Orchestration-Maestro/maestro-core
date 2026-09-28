//! Independent retrieval routes, fusion and reranking for knowledge search.

mod admission;
mod candidates;
mod deadline;
pub use deadline::DISABLED_BY_CONFIGURATION;
/// Public evidence assembly for ranked T029c search handoffs.
pub mod evidence;
mod filter;
mod fusion;
mod inventory_query;
mod orchestrate;
mod pin;
mod query;
mod request;
mod rerank;
mod route_execution;
pub mod routes;
#[cfg(test)]
mod tests;

pub use fusion::{Fused, Hit, Route, RouteList, fuse, fuse_weighted};
pub use orchestrate::search;
pub use pin::pin;
pub use query::Query;
pub use request::{
    EvidenceInput, SearchConfiguration, SearchContext, SearchError, SearchObservations,
    SearchRequest,
};
pub use rerank::{Candidate, Ranked, Reranked, Reranker, rerank};
pub use routes::outcome::{RouteOutcome, StructuredOutcome};
