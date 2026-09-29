//! Deterministic, digest-bound /4 delivery records.
use crate::{dedup::Deduplication, document::CanonicalDocument};
use serde::{Deserialize, Serialize};

/// One source revision with trusted family namespace context.
#[derive(Clone, Copy, Debug)]
pub struct UnitGraphInput<'a> {
    /// Canonical record.
    pub document: &'a CanonicalDocument,
    /// Original UTF-8 Markdown.
    pub markdown: &'a str,
    /// Trusted collection identity.
    pub collection_id: &'a str,
    /// Explicit namespace; empty values keep families local to the document.
    pub source_namespace: &'a str,
}

/// Delivery content kind; groups are not embedding points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    /// Complete section delivery unit.
    Section,
    /// Complete table delivery unit.
    Table,
    /// One row or bounded whole-row group.
    Row,
    /// Complete procedure or bounded whole-step group.
    Procedure,
    /// Code with its lead-in and associated small explanation.
    Code,
    /// Coherent consecutive paragraph group.
    Paragraphs,
    /// Other indivisible canonical source contribution.
    Block,
}

/// Role a source use plays in a delivery unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartRole {
    /// Primary source contribution owned exactly once.
    Primary,
    /// Reused heading, header, caption or introduction dependency.
    RequiredContext,
}

/// Exact byte range in original Markdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRange {
    /// Inclusive UTF-8 byte offset.
    pub start: usize,
    /// Exclusive UTF-8 byte offset.
    pub end: usize,
}

/// Canonical mapping contribution and its source coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingContribution {
    /// Canonical source unit identity.
    pub unit_id: String,
    /// Derived unit-text byte range.
    pub derived_range: (usize, usize),
    /// Source mapping mode from canonicalization.
    pub mapping_mode: String,
}

/// Exact boundary for an unsplittable oversize contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SplitMarker {
    /// Unit is complete and within its configured limit.
    Whole,
    /// A continuation cut at a verified structural boundary.
    Continuation {
        /// Zero-based continuation index.
        ordinal: usize,
        /// Number of continuations in this indivisible source item.
        total: usize,
    },
}

/// One complete delivery unit and all exact source uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryUnit {
    /// Stable digest-derived identifier.
    pub unit_id: String,
    /// Semantic unit kind.
    pub kind: UnitKind,
    /// Canonical block that owns the unit's primary contributions.
    pub source_block_id: String,
    /// Section identity, when nested in a section.
    pub section_id: Option<String>,
    /// Actual canonical heading ancestry.
    pub heading_path: Vec<String>,
    /// Occurrence among same-kind siblings.
    pub occurrence: usize,
    /// Innermost structural parent.
    pub parent_id: Option<String>,
    /// Ordered source parts and mapping contributions.
    pub parts: Vec<SourcePart>,
    /// Whether this unit is a complete source structure or continuation.
    pub split: SplitMarker,
}

/// Typed relation for a required source-context use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRelation {
    /// Table header required by its selected rows.
    HeaderToTable,
    /// Caption source required by its table.
    CaptionToTable,
    /// Procedure or code lead-in required by its content.
    LeadIn,
}

/// Explicit group relation to a required context part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRelationRecord {
    /// Group requiring this context.
    pub group_id: String,
    /// Typed source-dependency relation.
    pub kind: ContextRelation,
    /// Existing source part reused as context.
    pub part_id: String,
    /// Zero-based order among this group's context relations.
    pub ordinal: usize,
}

/// One exact use of original-source bytes in a delivery unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePart {
    /// Stable part identity.
    pub part_id: String,
    /// Primary ownership or repeated required context.
    pub role: PartRole,
    /// Zero-based order in this unit.
    pub ordinal: usize,
    /// One or more disjoint original Markdown ranges.
    pub ranges: Vec<SourceRange>,
    /// Exact contributing canonical mappings.
    pub mappings: Vec<MappingContribution>,
}

/// One original-source contribution paired with its canonical mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingEntry {
    /// Exact original Markdown range.
    pub range: SourceRange,
    /// Canonical mapping contribution.
    pub mapping: MappingContribution,
}

/// Separate versioned canonical mapping CAS artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingArtifact {
    /// Wire-schema version.
    pub schema_version: String,
    /// SHA-256 of original Markdown bytes.
    pub original_markdown_digest: String,
    /// Ordered exact source-to-derived contributions.
    pub contributions: Vec<MappingEntry>,
    /// Explicit noneligible source ranges and reasons.
    pub exclusions: Vec<Exclusion>,
}

/// One owned primary mapping in the source coverage ledger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageEntry {
    /// Primary source range.
    pub range: SourceRange,
    /// Owning delivery unit.
    pub unit_id: String,
    /// Owning source part.
    pub part_id: String,
}

/// Explicitly excluded source bytes and reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    /// Excluded original source range.
    pub range: SourceRange,
    /// Stable source-accounting disposition reason.
    pub reason: String,
}

/// Exact links from one ranked chunk/view to delivery units.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievalMembership {
    /// Delivery unit identity.
    pub unit_id: String,
    /// Exact primary part IDs represented by this view.
    pub primary_part_ids: Vec<String>,
    /// Disjoint exact primary source ranges represented by this view.
    pub primary_ranges: Vec<SourceRange>,
    /// Required-context part IDs, separate from primary contributions.
    pub context_part_ids: Vec<String>,
    /// Exact source ranges used only as required context.
    pub context_ranges: Vec<SourceRange>,
}

/// One retrieval representation of indexed content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievalView {
    /// Pinned ranking-unit value.
    pub rank_policy: String,
    /// Stable view identity.
    pub retrieval_view_id: String,
    /// Existing /3-compatible chunk identity.
    pub chunk_id: String,
    /// Digest of the prepared model input.
    pub prepared_input_digest: String,
    /// Verified token count of prepared input.
    pub token_count: usize,
    /// Ordered exact unit memberships.
    pub memberships: Vec<RetrievalMembership>,
}

/// Nonembedded ancestry group in source order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    /// Whole row or bounded row group.
    Row,
    /// Table parent.
    Table,
    /// Split procedure parent.
    Procedure,
    /// Code block with its direct lead-in.
    Code,
    /// Section parent.
    Section,
    /// Page root.
    Page,
    /// Paragraph set.
    Paragraphs,
}

/// Exact family comparison key; namespace must be supplied by a trusted caller.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyKey {
    /// Collection identity.
    pub collection_id: String,
    /// Caller-supplied source namespace.
    pub source_namespace: String,
    /// Last path segment only.
    pub path_segment: String,
    /// Normalized canonical page title.
    pub page_title: String,
    /// Normalized canonical heading path.
    pub heading_path: Vec<String>,
    /// Structural occurrence index.
    pub occurrence: usize,
}

/// Parent group with ordered children and exact dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// Stable identity.
    pub group_id: String,
    /// Group kind.
    pub kind: GroupKind,
    /// Section identity, if any.
    pub section_id: Option<String>,
    /// Parent group identity, if nested.
    pub parent_id: Option<String>,
    /// Ordered unit/group children.
    pub children: Vec<String>,
    /// Exact source parts and dependencies.
    pub parts: Vec<SourcePart>,
    /// Structural comparison family.
    pub family: FamilyKey,
}

/// Immutable graph identity and digest metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDescriptor {
    /// Wire-schema version.
    pub schema_version: String,
    /// Collection/source namespace.
    pub collection_id: String,
    /// Caller-supplied source namespace.
    pub source_namespace: String,
    /// Canonical document identity.
    pub document_id: String,
    /// Immutable revision identity.
    pub revision_id: String,
    /// Original Markdown SHA-256.
    pub original_markdown_digest: String,
    /// Chunk/profile name.
    pub profile_name: String,
    /// Complete profile rules digest.
    pub profile_digest: String,
    /// Preparation policy name.
    pub preparation_name: String,
    /// Preparation policy digest.
    pub preparation_digest: String,
    /// Verified token-counter contract.
    pub counter_contract: String,
    /// CAS digest of the serialized artifact, carried outside its JSON bytes.
    #[serde(skip)]
    pub graph_digest: String,
    /// Canonical source mapping digest.
    pub mapping_digest: String,
}

/// All independent delivery parts and ranking views for one authorized batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UnitBatch<'a> {
    /// Rules for this batch.
    pub profile: super::profile::UnitProfile,
    /// Exact occurrences retained by the existing authorization/grouping boundary.
    pub deduplication: Deduplication<'a>,
    /// Per-document delivery graphs in source identity order.
    pub graphs: Vec<DeliveryGraph>,
    /// Separate mapping artifacts in matching document order.
    pub mappings: Vec<MappingArtifact>,
}

/// Graph payload whose digest is carried by its descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryGraph {
    /// Graph identity and outside-payload digests.
    pub descriptor: GraphDescriptor,
    /// Delivery units in source order.
    pub units: Vec<DeliveryUnit>,
    /// Exactly-once ownership of eligible primary source ranges.
    pub coverage: Vec<CoverageEntry>,
    /// Explicit source exclusions with reasons.
    pub exclusions: Vec<Exclusion>,
    /// Retrieval representations.
    pub retrieval_views: Vec<RetrievalView>,
    /// Nonembedded groups in source order.
    pub groups: Vec<Group>,
    /// Typed group-to-context part relations.
    pub context_relations: Vec<ContextRelationRecord>,
}
