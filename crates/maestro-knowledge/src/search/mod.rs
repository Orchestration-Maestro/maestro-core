//! Independent retrieval routes, fusion and reranking for knowledge search.

mod filter;
mod fusion;
mod pin;
mod query;
mod request;
mod rerank;
pub mod routes;
#[cfg(test)]
mod tests;

pub use fusion::{Fused, Hit, Route, RouteList, fuse};
pub use pin::pin;
pub use query::Query;
pub use request::EvidenceInput;
pub use rerank::{Candidate, DEFAULT_DEPTH, Ranked, Reranked, Reranker, rerank};
