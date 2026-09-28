//! Dense and lexical routes against the shared fake and, when configured, real Qdrant.

use super::{backends, kernel, models};

mod chunk_set_documents;
mod cold_reranker;
mod configured_ask;
mod configured_search;
mod fused_search;
mod fused_search_admission_pinning;
mod identifier_route;
mod identifier_route_resilience;
mod idle_unload;
mod route_behavior;
mod route_errors;
mod scope_index;
mod search_projection;
mod structured_route;
pub(super) mod support;
