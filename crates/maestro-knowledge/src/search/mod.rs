//! Independent retrieval routes, fusion and reranking for knowledge search.

mod admission;
mod candidates;
mod deadline;
/// Internal evidence helpers, wired to the public entry point in the next T032 commit.
#[expect(
    dead_code,
    reason = "wired by the evidence entry point, the next T032 commit"
)]
pub(crate) mod evidence;
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

pub use fusion::{Fused, Hit, Route, RouteList, fuse};
pub use orchestrate::search;
pub use pin::pin;
pub use query::Query;
pub use request::{EvidenceInput, SearchContext, SearchError, SearchRequest};
pub use rerank::{Candidate, DEFAULT_DEPTH, Ranked, Reranked, Reranker, rerank};
pub use routes::outcome::{RouteOutcome, StructuredOutcome};
