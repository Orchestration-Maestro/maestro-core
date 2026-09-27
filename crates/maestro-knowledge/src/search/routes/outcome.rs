//! A route's hits and its independent availability status.

use super::results::ScoredChunk;
use maestro_kernel::evidence::{Inventory, RouteStatus};

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
