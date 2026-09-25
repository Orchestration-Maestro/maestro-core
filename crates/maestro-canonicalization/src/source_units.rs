//! Mapped source units: derived-text ranges and how each run relates to original source.
use crate::model::SourceSpan;
use serde::Serialize;

/// Version of canonical mapping, structural splitting and chunk identity rules.
pub const CHUNKER_VERSION: &str = "mapped-structural-chunks/2";

/// Half-open UTF-8 byte range in derived text, never original Markdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct TextRange {
    /// Inclusive derived-text byte offset.
    pub start: usize,
    /// Exclusive derived-text byte offset.
    pub end: usize,
}

/// Why a derived text run can be related to original source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginMode {
    /// Byte equality verified against one original source span.
    ExactCopy,
    /// Derived syntax/text; original spans must not be narrowed by rendered offsets.
    CanonicalTransformation,
    /// Generated layout without a source quotation or source-content coverage.
    Formatting,
}

/// One source-syntax origin, potentially broader than the derived contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceOrigin {
    /// Owning canonical block in the retained document revision.
    pub block_id: String,
    /// Original Markdown coordinates, suitable for exact source quotations.
    pub span: SourceSpan,
}

/// A contiguous run in a unit, fragment or input part's own text coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MappingRun {
    /// Derived-text coordinates relative to the containing record.
    pub range: TextRange,
    /// Exact, transformed or generated relation to source.
    pub mode: OriginMode,
    /// All contributing syntax origins; empty only for generated layout.
    pub origins: Vec<SourceOrigin>,
}

/// Typed location within an owning canonical block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UnitField {
    /// Direct inline root followed by any nested inline child indexes.
    Inline {
        /// First index addresses `StructuredContent::children`.
        child_path: Vec<usize>,
    },
    /// Full supplied code info string, not a guessed language.
    CodeInfo,
    /// Canonical list marker and actual ordered-list ordinal.
    ListMarker,
    /// Supplied quotation alert label.
    QuoteAlert,
    /// Supplied named footnote label.
    FootnoteLabel,
}

/// Meaning-bearing inline wrappers to preserve around split continuations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InlineEnvelope {
    /// Opening wrapper in its source unit's rendered coordinates.
    pub opening: TextRange,
    /// Closing wrapper in the same coordinates.
    pub closing: TextRange,
}

/// One nonduplicated inline contribution or context-only structural attribute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceUnit {
    /// Deterministic identity from chunker version, block identity and typed field.
    pub unit_id: String,
    /// Owning canonical block.
    pub block_id: String,
    /// Exact typed location, not a flattened block-text offset.
    pub field: UnitField,
    /// True for eligible primary body content, false for structural context only.
    pub primary: bool,
    /// Complete canonical rendering, without context or inter-block layout.
    pub text: String,
    /// Exhaustive partition of this unit's text into mapped runs.
    pub mappings: Vec<MappingRun>,
    /// Nested semantic wrappers, not presentation-only emphasis.
    pub envelopes: Vec<InlineEnvelope>,
}

/// Disposition of an entry in the separate original-byte accounting ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDisposition {
    /// Parsed or raw contribution represented in primary mapped units.
    Eligible,
    /// Delimiters and inter-block whitespace, retained without standalone body text.
    Structural,
    /// Nonstandalone metadata retained in the canonical source record.
    Metadata,
    /// A retained declaration, not standalone searchable body content.
    ReferenceDefinition,
}

/// Reconciliation of one original ledger entry with mapped units or exclusions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AccountingDisposition {
    /// Index into the retained canonical document's original-byte ledger.
    pub accounting_index: usize,
    /// Eligibility or explicit exclusion category.
    pub disposition: SourceDisposition,
    /// Referencing units; overlap is legitimate and does not imply body coverage.
    pub unit_indices: Vec<usize>,
}

/// Canonical mapped representation before any token budget is applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MappedDocument {
    /// Units in canonical traversal order, preserving inline/block interleaving.
    pub units: Vec<SourceUnit>,
    /// One disposition per original source-accounting entry.
    pub accounting: Vec<AccountingDisposition>,
}
