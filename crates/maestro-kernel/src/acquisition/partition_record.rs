//! Pending checkpoints are separate from immutable accepted partition snapshots.
use super::{privacy::Handle, record::NewItem};
use crate::artifact::Digest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Enumeration contract, never keyword search or guessed identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Enumeration {
    /// A declared offline captured-HTML link contract, recorded with its exact extractor.
    Links,
    /// A source-provided bounded index with explicit coverage evidence.
    Index,
}
/// Finite source window; overlap and skew never imply remote snapshot stability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    /// Inclusive source lower bound.
    pub start: u64,
    /// Committable upper watermark.
    pub end: u64,
    /// Declared overlap with earlier windows.
    pub overlap: u64,
    /// Declared source clock uncertainty.
    pub skew: u64,
}
/// Frozen bounded partition identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Partition {
    /// Opaque unique partition; never reused for another run or window.
    pub id: Handle,
    /// Logical run identity.
    pub run: Handle,
    /// Declared enumeration kind.
    pub kind: Enumeration,
    /// Exact source bounds.
    pub window: Window,
    /// Finite batch ceiling (1–1,000); batch and item ceilings sum to at most 1,000.
    pub max_batches: u16,
    /// Finite distinct-item ceiling (1–1,000).
    pub max_items: u16,
}
/// Source change signals, independent of visible-text equality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeKeys {
    /// Source revision, unknown when the source provides none.
    pub revision: Option<Digest>,
    /// Safe validator evidence, never raw remote secrets.
    pub validator: Option<Digest>,
    /// Source metadata, unknown when unavailable.
    pub metadata: Option<Digest>,
    /// Exact effective permission identity.
    pub permissions: Digest,
    /// Exact canonical link inventory digest.
    pub links: Digest,
    /// Raw representation identity; hidden link changes remain revisions.
    pub representation: Digest,
}
/// Eligible request handed to the existing frontier, never a competing queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredItem {
    /// Existing request/context uniqueness key.
    pub request: NewItem,
    /// Immutable non-text source signals.
    pub keys: ChangeKeys,
}
/// Immutable pending checkpoint and exact coverage evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    /// Frozen partition descriptor.
    pub partition: Partition,
    /// Entire typed source continuation, not a string projection.
    pub cursor: Option<Value>,
    /// Entire next continuation; empty items do not imply terminal.
    pub next: Option<Value>,
    /// Explicit terminal evidence from the enumeration contract.
    pub terminal: bool,
    /// Explicit source stability evidence.
    pub stable: bool,
    /// A cap or truncation prevents complete coverage.
    pub truncated: bool,
    /// Exact expected eligible batch count; unknown cannot prove coverage.
    pub expected: Option<u16>,
    /// Every eligible item and its independent change keys.
    pub items: Vec<DiscoveredItem>,
    /// Exact extractor/version/function/selector contract; absent for indexes.
    pub extractor: Option<String>,
    /// Candidate identities excluded by current source policy, hashed for safety.
    pub denied: Vec<Digest>,
    /// Prepared immutable parent capture, absent for source indexes.
    pub capture: Option<Handle>,
}
/// An accepted snapshot exists only after all checkpoint items are captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedPartition {
    /// Committed complete upper source bound.
    pub watermark: u64,
}
/// Protected view of separate pending and accepted evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionState {
    /// Immutable checkpoints in continuation order.
    pub batches: Vec<Batch>,
    /// Current distinct items still lacking acknowledged capture linkage.
    pub pending: u16,
    /// Separate immutable snapshot; no speculative watermark.
    pub accepted: Option<AcceptedPartition>,
}
