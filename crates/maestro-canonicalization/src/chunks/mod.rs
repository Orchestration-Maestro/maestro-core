//! Derived Phase B coordinates and evidence; none of these records grant access.
use self::identity::{PreparedGroups, chunk_id, insert_prepared_group, prepared_identity};
use self::validation::{validate_chunks, validate_coverage};
use crate::{
    DedupInput, DedupScope, Deduplication, Error, NativeTokenizer, SourceSpan, WarningPolicy,
    chunk_mapping::map_document,
    chunk_split::{MAX_TOKENS, TARGET_TOKENS, build_drafts},
};
use serde::Serialize;
use std::collections::BTreeMap;

mod identity;
#[cfg(test)]
mod tests;
mod validation;

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

/// Explicit role of a piece of the complete embedding input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRole {
    /// Nonoverlapping primary rendered content.
    SourceContent,
    /// Repeated actual heading ancestry.
    HeadingContext,
    /// Corresponding column labels for selected table windows.
    TableHeaderContext,
    /// Full directly owned ancestor-item content, not nested item subtrees.
    ParentListContext,
    /// Supplied structural attributes or repeated meaning-bearing wrappers.
    StructuralContext,
    /// Enumerated generated layout, never an original quotation.
    FormattingSeparator,
}

/// A selection in one mapped source unit's derived coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Contribution {
    /// Index into the document's mapped units.
    pub unit_index: usize,
    /// Original unit-relative range, not part-relative or Markdown-relative.
    pub range: TextRange,
}

/// One verbatim-concatenated piece of the prepared input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputPart {
    /// Why this text is included.
    pub role: InputRole,
    /// Exact bytes sent to the qualified tokenizer.
    pub text: String,
    /// Location in the complete prepared input.
    pub prepared_range: TextRange,
    /// Source-unit selections; context copies do not add primary coverage.
    pub contributions: Vec<Contribution>,
    /// Mappings in this part's own text coordinates.
    pub mappings: Vec<MappingRun>,
    /// True only for generated formatting inside the primary body.
    pub body_layout: bool,
}

/// Honest continuation boundary, without claiming linguistic completeness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitKind {
    /// Complete natural contribution.
    Whole,
    /// Structural subdivision of a larger container.
    Structural,
    /// Punctuation-plus-whitespace heuristic, not sentence understanding.
    Sentence,
    /// Whitespace boundary.
    Whitespace,
    /// Unicode scalar boundary, not a byte cut or overlap.
    Scalar,
    /// Code-line boundary.
    CodeLine,
    /// Explicitly within a code line.
    CodeLineFragment,
    /// Explicitly within a table cell.
    CellFragment,
}

/// One primary unit selection; repeated contexts never become fragments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Fragment {
    /// Original mapped unit selection.
    pub contribution: Contribution,
    /// Zero-based continuation ordinal for this unit across the document.
    pub part_ordinal: usize,
    /// Structural or textual boundary used for this piece.
    pub split: SplitKind,
}

/// Explicit table membership, including which columns were actually selected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TableWindow {
    /// Canonical table identity.
    pub table_id: String,
    /// Canonical row identity, not invented flattened-row numbering.
    pub row_id: String,
    /// Zero-based row position, with the header at zero.
    pub row_index: usize,
    /// Selected zero-based columns; omitted columns are not fabricated blanks.
    pub columns: Vec<usize>,
}

/// Prepared chunk body, context, exact count and structural provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChunkContent {
    /// Actual owning section; a heading owns the section it introduces.
    pub section_id: Option<String>,
    /// Canonical structural containers, retaining all source relationships.
    pub container_ids: Vec<String>,
    /// Actual source heading titles; no metadata title is injected.
    pub heading_path: Vec<String>,
    /// Nonoverlapping primary contributions.
    pub fragments: Vec<Fragment>,
    /// Explicit row/column windows for table content.
    pub table_windows: Vec<TableWindow>,
    /// Primary body plus its generated layout, excluding contextual copies.
    pub body_text: String,
    /// Complete input serialization, with every separator represented.
    pub input_parts: Vec<InputPart>,
    /// Verbatim concatenation of all input parts.
    pub prepared_input: String,
    /// Complete-input count; only the verified native API certifies this value.
    pub token_count: usize,
}

/// Version of context and formatting preparation rules.
pub const PREPARATION_PROFILE: &str = "canonical-context-parts/v1";

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
    /// Qualified local counter identity.
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

/// Prepare structural chunks with the qualified, vocabulary-only native counter.
/// Fresh authorization and canonical replay are required for every call.
///
/// # Errors
/// Refuses unauthorized/repeated revisions, invalid sources, disallowed warnings,
/// unsafe splits, impossible context budgets, incomplete coverage or changed artifacts.
pub fn chunk_documents<'a>(
    scope: &'a DedupScope,
    inputs: &[DedupInput<'a>],
    warning_policy: WarningPolicy,
    tokenizer: &NativeTokenizer,
) -> Result<ChunkBatch<'a>, Error> {
    let deduplication = crate::group_exact(scope, inputs, warning_policy)?;
    tokenizer.verify_artifacts()?;
    let batch = build_batch(deduplication, tokenizer.contract_id(), &mut |input| {
        Ok(tokenizer.token_ids(input)?.len())
    })?;
    tokenizer.verify_artifacts()?;
    Ok(batch)
}

#[cfg(test)]
fn chunk_with_count<'a>(
    scope: &'a DedupScope,
    inputs: &[DedupInput<'a>],
    warning_policy: WarningPolicy,
    tokenizer_contract_id: &str,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<ChunkBatch<'a>, Error> {
    build_batch(
        crate::group_exact(scope, inputs, warning_policy)?,
        tokenizer_contract_id,
        count,
    )
}

/// Map, chunk, validate and identify every authorized occurrence, counting each distinct prepared
/// input once per batch.
fn build_batch<'a>(
    deduplication: Deduplication<'a>,
    tokenizer_contract_id: &str,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<ChunkBatch<'a>, Error> {
    // ponytail: full-string, batch-local cache; cap/evict if measured authorized
    // batches outgrow RAM.
    let mut cache = BTreeMap::new();
    let mut cached_count = |input: &str| -> Result<usize, Error> {
        if let Some(&tokens) = cache.get(input) {
            return Ok(tokens);
        }
        let tokens = count(input)?;
        cache.insert(input.to_owned(), tokens);
        Ok(tokens)
    };
    let mut documents = Vec::new();
    let mut chunks = Vec::new();
    let mut groups = PreparedGroups::new();
    for (occurrence_index, occurrence) in deduplication.occurrences.iter().enumerate() {
        let doc = occurrence.document;
        let mapped = map_document(doc, occurrence.markdown)?;
        let drafts = build_drafts(doc, occurrence.markdown, &mapped, &mut cached_count)?;
        let coverage = validate_coverage(&mapped, &drafts)?;
        validate_chunks(
            doc,
            occurrence.markdown,
            &mapped,
            &drafts,
            &mut cached_count,
        )?;
        for (ordinal, content) in drafts.into_iter().enumerate() {
            let chunk_id = chunk_id(
                &deduplication,
                doc,
                &mapped,
                &content,
                tokenizer_contract_id,
            )?;
            let prepared = prepared_identity(
                &deduplication,
                tokenizer_contract_id,
                &content.prepared_input,
            )?;
            insert_prepared_group(
                &mut groups,
                prepared.fingerprint.clone(),
                prepared.bytes,
                prepared.group_id,
                chunks.len(),
            )?;
            chunks.push(RetrievalChunk {
                chunk_id,
                occurrence_index,
                ordinal,
                content,
                retrieval_input_fingerprint: prepared.fingerprint,
            });
        }
        documents.push(ChunkDocument {
            occurrence_index,
            no_searchable_content: coverage.is_empty(),
            mapped,
            coverage,
        });
    }
    Ok(ChunkBatch {
        version: CHUNKER_VERSION.into(),
        preparation_profile: PREPARATION_PROFILE.into(),
        tokenizer_contract_id: tokenizer_contract_id.into(),
        target_tokens: TARGET_TOKENS,
        hard_max_tokens: MAX_TOKENS,
        overlap_tokens: 0,
        deduplication,
        documents,
        chunks,
        prepared_groups: groups.into_values().map(|(_, group)| group).collect(),
    })
}

/// The JSON bytes an identity digests.
fn record_bytes(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(|_| invalid_chunks())
}

/// The refusal for chunks whose coverage or preparation does not replay.
fn invalid_chunks() -> Error {
    Error("invalid chunk coverage or preparation".into())
}
