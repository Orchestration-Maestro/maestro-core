//! Private serde DTOs for the versioned graph and mapping payloads.
#![allow(
    clippy::missing_docs_in_private_items,
    reason = "Private DTO fields mirror the fully documented, pinned wire contract."
)]
use super::types::{
    ContextRelation, Exclusion, FamilyKey, GroupKind, MappingContribution, SourceRange,
    SplitMarker, UnitKind,
};
use serde::{Deserialize, Serialize};

/// Graph descriptor shape accepted by the strict wire contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireGraphDescriptor {
    pub schema_version: String,
    pub collection_id: String,
    pub source_namespace: String,
    pub document_id: String,
    pub revision_id: String,
    pub original_markdown_digest: String,
    pub profile_name: String,
    pub profile_digest: String,
    pub preparation_name: String,
    pub preparation_digest: String,
    pub counter_contract: String,
    pub mapping_digest: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireSourceRange {
    pub start: usize,
    pub end: usize,
}

impl From<SourceRange> for WireSourceRange {
    fn from(range: SourceRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireMappingContribution {
    pub unit_id: String,
    pub derived_range: (usize, usize),
    pub mapping_mode: String,
}

impl From<&MappingContribution> for WireMappingContribution {
    fn from(mapping: &MappingContribution) -> Self {
        Self {
            unit_id: mapping.unit_id.clone(),
            derived_range: mapping.derived_range,
            mapping_mode: mapping.mapping_mode.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireExclusion {
    pub range: WireSourceRange,
    pub reason: String,
}

impl From<&Exclusion> for WireExclusion {
    fn from(exclusion: &Exclusion) -> Self {
        Self {
            range: exclusion.range.into(),
            reason: exclusion.reason.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireMappingEntry {
    pub range: WireSourceRange,
    pub mapping: WireMappingContribution,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireMappingArtifact {
    pub schema_version: String,
    pub original_markdown_digest: String,
    pub contributions: Vec<WireMappingEntry>,
    pub exclusions: Vec<WireExclusion>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum WireSplitMarker {
    Whole,
    Continuation { ordinal: usize, total: usize },
}

impl From<&SplitMarker> for WireSplitMarker {
    fn from(marker: &SplitMarker) -> Self {
        match marker {
            SplitMarker::Whole => Self::Whole,
            SplitMarker::Continuation { ordinal, total } => Self::Continuation {
                ordinal: *ordinal,
                total: *total,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WireContextRelationKind {
    HeaderToTable,
    CaptionToTable,
    LeadIn,
}

impl From<ContextRelation> for WireContextRelationKind {
    fn from(relation: ContextRelation) -> Self {
        match relation {
            ContextRelation::HeaderToTable => Self::HeaderToTable,
            ContextRelation::CaptionToTable => Self::CaptionToTable,
            ContextRelation::LeadIn => Self::LeadIn,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WireGroupKind {
    Row,
    Table,
    Procedure,
    Code,
    Section,
    Page,
    Paragraphs,
}

impl From<GroupKind> for WireGroupKind {
    fn from(kind: GroupKind) -> Self {
        match kind {
            GroupKind::Row => Self::Row,
            GroupKind::Table => Self::Table,
            GroupKind::Procedure => Self::Procedure,
            GroupKind::Code => Self::Code,
            GroupKind::Section => Self::Section,
            GroupKind::Page => Self::Page,
            GroupKind::Paragraphs => Self::Paragraphs,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireFamilyKey {
    pub collection_id: String,
    pub source_namespace: String,
    pub path_segment: String,
    pub page_title: String,
    pub heading_path: Vec<String>,
    pub occurrence: usize,
}

impl From<&FamilyKey> for WireFamilyKey {
    fn from(family: &FamilyKey) -> Self {
        Self {
            collection_id: family.collection_id.clone(),
            source_namespace: family.source_namespace.clone(),
            path_segment: family.path_segment.clone(),
            page_title: family.page_title.clone(),
            heading_path: family.heading_path.clone(),
            occurrence: family.occurrence,
        }
    }
}

/// Revised strict graph wire DTO; producer-only metadata stays in `DeliveryGraph`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireGraph {
    /// Graph identity and external artifact digests.
    pub descriptor: WireGraphDescriptor,
    /// Unique source parts in source order.
    pub parts: Vec<WirePart>,
    /// Delivery units in source order.
    pub units: Vec<WireUnit>,
    /// Explicit source exclusions.
    pub exclusions: Vec<WireExclusion>,
    /// Retrieval representations.
    pub retrieval_views: Vec<WireRetrievalView>,
    /// Nonembedded source ancestry.
    pub groups: Vec<WireGroup>,
}

/// Sole wire owner of ranges and canonical mappings for one part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WirePart {
    /// Stable source-part identity.
    pub part_id: String,
    /// Exact original-source byte ranges.
    pub ranges: Vec<WireSourceRange>,
    /// Canonical source mapping contributions.
    pub mappings: Vec<WireMappingContribution>,
}

/// Delivery-unit references without duplicating source ranges or mapping records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireUnit {
    /// Stable unit identity.
    pub unit_id: String,
    /// Contract unit kind.
    pub kind: WireUnitKind,
    /// Parent group identity.
    pub parent: Option<String>,
    /// Ordered primary source-part identities.
    pub part_ids: Vec<String>,
    /// Whole unit or verified continuation marker.
    pub split: WireSplitMarker,
}

/// Unit kinds accepted by graph contract v1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WireUnitKind {
    /// Complete section delivery unit.
    Section,
    /// Complete table delivery unit.
    Table,
    /// One row or bounded whole-row group.
    Row,
    /// Complete procedure or bounded whole-step group.
    Procedure,
    /// Code with its lead-in and associated explanation.
    Code,
    /// Coherent consecutive paragraph group.
    Paragraphs,
    /// Other indivisible canonical contribution.
    Block,
}

impl From<UnitKind> for WireUnitKind {
    fn from(kind: UnitKind) -> Self {
        match kind {
            UnitKind::Section => Self::Section,
            UnitKind::Table => Self::Table,
            UnitKind::Row => Self::Row,
            UnitKind::Procedure => Self::Procedure,
            UnitKind::Code => Self::Code,
            UnitKind::Paragraphs => Self::Paragraphs,
            UnitKind::Block => Self::Block,
        }
    }
}

/// Retrieval view and the primary part IDs it represents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireRetrievalView {
    /// Pinned ranking-unit rule.
    pub rank_policy: String,
    /// Stable retrieval-view identity.
    pub retrieval_view_id: String,
    /// Existing /3-compatible chunk identity.
    pub chunk_id: String,
    /// Prepared model-input digest.
    pub prepared_input_digest: String,
    /// Verified prepared-input token count.
    pub token_count: usize,
    /// Ordered unit memberships.
    pub memberships: Vec<WireMembership>,
}

/// One delivery-unit's primary parts in a retrieval view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireMembership {
    /// Delivery unit identity.
    pub unit_id: String,
    /// Primary source parts represented by this view.
    pub primary_part_ids: Vec<String>,
}

/// Typed group dependency with ordinal carried by array position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireContextRelation {
    /// Required-context dependency kind.
    pub kind: WireContextRelationKind,
    /// Existing source-part identity directly owned by the group.
    pub part_id: String,
}

/// Group ancestry with references to top-level source parts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireGroup {
    /// Stable group identity.
    pub group_id: String,
    /// Structural group kind.
    pub kind: WireGroupKind,
    /// Parent group identity.
    pub parent: Option<String>,
    /// Ordered unit or group children.
    pub children: Vec<String>,
    /// Ordered direct source-part IDs.
    pub part_ids: Vec<String>,
    /// Optional heading part.
    pub heading: Option<String>,
    /// Required context, with ordinal given by array position.
    pub context_relations: Vec<WireContextRelation>,
    /// Structural family key.
    pub family: WireFamilyKey,
}
