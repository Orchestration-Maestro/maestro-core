//! Independent retrieval routes, fusion and reranking for knowledge search.

mod admission;
mod candidates;
mod deadline;
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

pub use evidence::{EvidenceCounter, EvidenceError, assemble_evidence};
pub use fusion::{Fused, Hit, Route, RouteList, fuse};
pub use orchestrate::search;
pub use pin::pin;
pub use query::Query;
pub use request::{EvidenceInput, SearchContext, SearchError, SearchRequest};
pub use rerank::{Candidate, DEFAULT_DEPTH, Ranked, Reranked, Reranker, rerank};
pub use routes::outcome::{RouteOutcome, StructuredOutcome};
