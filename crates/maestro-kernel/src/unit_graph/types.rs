//! Kernel-owned delivery graph wire values, independent of canonicalization.
use crate::artifact::Digest;
use serde::{Deserialize, Serialize};

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

/// Exact byte range in original Markdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRange {
    /// Inclusive UTF-8 byte offset.
    pub start: u64,
    /// Exclusive UTF-8 byte offset.
    pub end: u64,
}

/// Canonical mapping contribution and its source coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingContribution {
    /// Canonical source unit identity.
    pub unit_id: String,
    /// Derived unit-text byte range.
    pub derived_range: (u64, u64),
    /// Source mapping mode from canonicalization.
    pub mapping_mode: String,
}

/// One graph-wide part owns its ranges and mappings exactly once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    /// Stable part identity.
    pub part_id: String,
    /// Ordered original-source ranges.
    pub ranges: Vec<SourceRange>,
    /// Mapping contributions paired with ranges.
    pub mappings: Vec<MappingContribution>,
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
        ordinal: u64,
        /// Number of continuations in this indivisible source item.
        total: u64,
    },
}

/// One complete delivery unit referring to primary source parts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryUnit {
    /// Stable digest-derived identifier.
    pub unit_id: String,
    /// Semantic unit kind.
    pub kind: UnitKind,
    /// Group containing this unit, when structural.
    pub parent: Option<String>,
    /// Ordered primary part IDs.
    pub part_ids: Vec<String>,
    /// Whether this unit is a complete source structure or continuation.
    pub split: SplitMarker,
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
    /// Ordered subset of the unit's primary parts represented by this view.
    pub primary_part_ids: Vec<String>,
}

/// One retrieval representation of indexed content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievalView {
    /// Pinned ranking-unit value.
    pub rank_policy: RankPolicy,
    /// Stable view identity.
    pub retrieval_view_id: String,
    /// Existing /3-compatible chunk identity.
    pub chunk_id: String,
    /// Digest of the prepared model input.
    pub prepared_input_digest: Digest,
    /// Verified token count of prepared input.
    pub token_count: u64,
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
    /// Section parent.
    Section,
    /// Page root.
    Page,
    /// Paragraph set.
    Paragraphs,
    /// Code block together with its directly owned lead-in.
    Code,
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
    pub occurrence: u64,
}

/// Context dependency kind carried by a structural group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextKind {
    /// Actual table header, not an inferred nearby paragraph.
    HeaderToTable,
    /// Actual table caption.
    CaptionToTable,
    /// Procedure introduction or code lead-in.
    LeadIn,
}

/// Typed context reference, scoped to its containing group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRelation {
    /// Structural purpose.
    pub kind: ContextKind,
    /// Owned direct part reused as context.
    pub part_id: String,
}

/// Parent group with ordered children and direct structural dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// Stable identity.
    pub group_id: String,
    /// Group kind.
    pub kind: GroupKind,
    /// Parent group identity, if nested.
    pub parent: Option<String>,
    /// Ordered unit/group children.
    pub children: Vec<String>,
    /// Parts directly owned by this group.
    pub part_ids: Vec<String>,
    /// Heading part ID, allowed only for section and page groups.
    pub heading: Option<String>,
    /// Typed context dependencies; ordinals are array order.
    pub context_relations: Vec<ContextRelation>,
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
    pub original_markdown_digest: Digest,
    /// Chunk/profile name.
    pub profile_name: String,
    /// Complete profile rules digest.
    pub profile_digest: Digest,
    /// Preparation policy name.
    pub preparation_name: String,
    /// Preparation policy digest.
    pub preparation_digest: Digest,
    /// Verified token-counter contract.
    pub counter_contract: String,
    /// Canonical source mapping digest.
    pub mapping_digest: Digest,
}

/// Graph payload whose digest is carried by its descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryGraph {
    /// Graph identity and outside-payload digests.
    pub descriptor: GraphDescriptor,
    /// Source-ordered graph-wide parts.
    pub parts: Vec<Part>,
    /// Delivery units in source order.
    pub units: Vec<DeliveryUnit>,
    /// Explicit source exclusions with reasons.
    pub exclusions: Vec<Exclusion>,
    /// Retrieval representations.
    pub retrieval_views: Vec<RetrievalView>,
    /// Nonembedded groups in source order.
    pub groups: Vec<Group>,
}

/// The digest-pinned ranked-unit rule, not a separate delivery pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankPolicy {
    /// Packed complete-idea retrieval views.
    CompleteIdeas,
    /// Bounded structural retrieval units.
    V2Unit,
}
