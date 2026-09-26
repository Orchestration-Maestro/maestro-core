//! Batch records: per-occurrence evidence, retrieval chunks and prepared-input groups.
use crate::dedup::Deduplication;
use crate::prepared_inputs::ChunkContent;
use crate::source_units::{MappedDocument, TextRange};
use serde::Serialize;

/// Exactly-once primary coverage in one mapped unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UnitCoverage {
    /// Index into the document's mapped units.
    pub unit_index: usize,
    /// Ordered, disjoint ranges covering the full eligible text.
    pub primary_ranges: Vec<TextRange>,
}

/// Source mapping and dual coverage evidence for one retained occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChunkDocument {
    /// Index into the retained deduplication occurrences.
    pub occurrence_index: usize,
    /// Canonical units and complete original-byte dispositions.
    pub mapped: MappedDocument,
    /// Primary rendered coverage; context copies are excluded.
    pub coverage: Vec<UnitCoverage>,
    /// Explicitly true when this document has no eligible body content.
    pub no_searchable_content: bool,
}

/// One immutable source occurrence's derived retrieval input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RetrievalChunk {
    /// Scope, source revision, profile and primary-coordinate identity.
    pub chunk_id: String,
    /// Index into the unchanged deduplication occurrences.
    pub occurrence_index: usize,
    /// Zero-based chunk position within that occurrence.
    pub ordinal: usize,
    /// Complete input, structural provenance and exact count.
    pub content: ChunkContent,
    /// Candidate equality hash, not authorization or proof of byte equality.
    pub retrieval_input_fingerprint: String,
}

/// Byte-verified equality of complete prepared inputs in one scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PreparedInputGroup {
    /// Scope/profile/content identity, independent of current membership.
    pub group_id: String,
    /// SHA-256 candidate verified against actual equality bytes.
    pub content_hash: String,
    /// All matching occurrences, including singletons.
    pub chunk_indices: Vec<usize>,
}

/// All-or-error result under a caller's authorization snapshot, not an access grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChunkBatch<'a> {
    /// Chunking and identity policy version.
    pub version: String,
    /// Canonical context/formatting policy.
    pub preparation_profile: String,
    /// Contract ID of the counter that counted the batch.
    pub tokenizer_contract_id: String,
    /// Preferred complete-input size, not a minimum.
    pub target_tokens: usize,
    /// Maximum complete-input size, including context and native specials.
    pub hard_max_tokens: usize,
    /// Primary body overlap; repeated context does not add coverage.
    pub overlap_tokens: usize,
    /// Every authorized source revision and its independent policies.
    pub deduplication: Deduplication<'a>,
    /// Per-occurrence mapping and accounting evidence.
    pub documents: Vec<ChunkDocument>,
    /// Ordered chunks without source identities being merged away.
    pub chunks: Vec<RetrievalChunk>,
    /// Scope-local equality classes, retaining every occurrence.
    pub prepared_groups: Vec<PreparedInputGroup>,
}
