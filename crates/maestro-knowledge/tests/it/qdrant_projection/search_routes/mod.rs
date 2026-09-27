//! Dense and lexical routes against the shared fake and, when configured, real Qdrant.

use super::{backends, kernel, models};

mod route_behavior;
mod route_errors;
mod scope_index;
mod support;
