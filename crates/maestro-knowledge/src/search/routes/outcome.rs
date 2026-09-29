//! A route's hits and its independent availability status.

use super::results::ScoredChunk;
use maestro_kernel::evidence::{Inventory, RouteStatus};
use serde::Serialize;

/// The partial or complete result of one independent search route.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteOutcome {
    /// Hits in this route's stable rank order.
    pub hits: Vec<ScoredChunk>,
    /// Whether the route completed, including all of its backend legs.
    pub status: RouteStatus,
}

/// Structured hits and, independently, the complete inventory they support.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuredOutcome {
    /// The route status and its bounded supporting chunks.
    pub route: RouteOutcome,
    /// The exact inventory, absent when the route failed.
    pub inventory: Option<Inventory>,
}

/// The identifier route's outcome and the identifiers its noise guard dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifierOutcome {
    /// The route status and its hits from the identifiers it kept.
    pub route: RouteOutcome,
    /// The identifiers kept out of both legs, in question order.
    pub dropped: Vec<DroppedIdentifier>,
}

/// An identifier the noise guard kept out of the identifier route, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DroppedIdentifier {
    /// The identifier as the question wrote it.
    pub identifier: String,
    /// Why it was dropped, such as `identifier too common`.
    pub reason: String,
}
