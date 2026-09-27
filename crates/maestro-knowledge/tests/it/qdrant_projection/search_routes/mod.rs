//! Dense and lexical routes against the shared fake and, when configured, real Qdrant.

use super::{backends, kernel, models};

mod fused_search;
mod identifier_route;
mod route_behavior;
mod route_errors;
mod scope_index;
mod search_projection;
mod structured_route;
pub(super) mod support;
