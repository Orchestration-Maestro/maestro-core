//! Public inputs and results of kernel-owned search operations.

use crate::{evidence::Inventory, generation::Generation, scope::ScopeSet};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

/// The exact identifier payload and kernel-search projection profile.
pub const IDENTIFIER_PROFILE: &str = "identifiers/2";

/// One exact prepared input to index for identifier search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInput {
    /// The chunk within its chunk set.
    pub chunk_id: String,
    /// The exact UTF-8 text whose digest the chunk records.
    pub prepared_input: String,
    /// Exact values emitted by the shared query and payload identifier extractor.
    pub identifiers: Vec<String>,
}

/// One document revision represented by a chunk-set revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMember {
    /// The member document's own revision.
    pub revision_id: String,
    /// Its chunk-owning representative revision, or itself.
    pub representative_revision_id: String,
}

/// The identifier-search projection attached to a generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchProjection {
    /// The exact profile used to extract its payload identifiers.
    pub identifier_profile: String,
    /// Whether publication verified every required search derivative.
    pub ready: bool,
}

/// A bounded exact inventory request over a pinned generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryRequest {
    /// Count generation members grouped by their text `set` metadata.
    DocumentsBySet {
        /// Restrict the population to one exact set value.
        set: Option<String>,
    },
    /// Count generation members grouped by their text `version` metadata.
    Versions {
        /// Restrict the population to one exact set value.
        set: Option<String>,
    },
}

/// A controlled synchronous read of a pinned generation.
#[derive(Debug)]
pub struct SearchRead<'a> {
    /// The generation pinned for this request.
    pub generation: &'a Generation,
    /// The immutable permission snapshot for this request.
    pub scopes: &'a ScopeSet,
    /// The exact version filter, if the request selected one.
    pub version: Option<&'a str>,
    /// The absolute deadline and cancellation flag for this read.
    pub control: &'a ReadControl,
}

/// A chunk and owning revision returned by a scoped search read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkHit {
    /// The chunk ID in the pinned chunk set.
    pub chunk_id: String,
    /// The revision that owns this chunk.
    pub revision_id: String,
}

/// Exact identifier hits and any high-frequency values skipped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdentifierSearchResult {
    /// Scoped, exact matches in stable search order.
    pub hits: Vec<ChunkHit>,
    /// The requested identifiers, in request order, that matched more chunks
    /// than the hit limit and were skipped.
    pub too_common: Vec<String>,
}

/// A complete inventory and its bounded supporting chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventorySelection {
    /// The exact, complete inventory for the selected population.
    pub inventory: Inventory,
    /// Up to twenty scoped chunks that support the inventory.
    pub supports: Vec<ChunkHit>,
}

/// Cancellation and an absolute wall-clock deadline for a blocking search read.
#[derive(Debug)]
pub struct ReadControl {
    /// The wall-clock instant after which the read must stop.
    pub deadline: Instant,
    /// Set when the async caller no longer waits for this worker.
    pub cancelled: Arc<AtomicBool>,
}
