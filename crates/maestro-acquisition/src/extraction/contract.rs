//! Shared mapped extraction contract. No parser or canonical preparation runs here.
use super::model::Processing;
use crate::Ref;
use maestro_kernel::{acquisition::Handle, artifact::Digest};
use serde::{Deserialize, Serialize};

/// Half-open byte span in the immutable source or the extracted Markdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteSpan {
    /// Inclusive byte offset.
    pub start: u64,
    /// Exclusive byte offset.
    pub end: u64,
}
impl ByteSpan {
    /// A span must fit its artifact; empty observed structures remain valid.
    #[must_use]
    pub fn fits(self, length: u64) -> bool {
        self.start <= self.end && self.end <= length
    }
    /// UTF-8 boundaries must also fit when checking Markdown correspondence.
    #[must_use]
    pub fn text(self, markdown: &str) -> Option<&str> {
        let start = usize::try_from(self.start).ok()?;
        let end = usize::try_from(self.end).ok()?;
        markdown.get(start..end)
    }
}

/// Required cohorts compare ordered correspondence and content, not just totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureKind {
    /// Heading text and order.
    Heading,
    /// Literal code, including indentation and case.
    Code,
    /// Table identity and structure.
    Table,
    /// Ordered table rows.
    Row,
    /// Ordered table cells.
    Cell,
    /// Ordered list items.
    List,
    /// Link destinations.
    Link,
    /// Image references.
    Image,
    /// Required immutable assets.
    Asset,
    /// Literal technical meaning, numbers, units, versions and identifiers.
    Literal,
    /// Negated technical claims.
    Negation,
    /// Required prerequisites.
    Prerequisite,
    /// Source warnings, distinct from extraction warnings.
    Warning,
    /// Other faithfully extracted text.
    Text,
}

/// Unavailable measurements never collapse to an observed empty inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Measured<T> {
    /// No trustworthy source measurement is available.
    Unknown,
    /// Observed value; an empty vector means known zero.
    Known(T),
}
/// Exact literal content or an immutable required asset identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Content {
    /// Exact source technical content; no model rewriting or case folding.
    Text(String),
    /// Required original asset reference.
    Asset(Ref),
}
/// One observed source unit, tied to the document's immutable source artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceUnit {
    /// Typed structure cohort.
    pub kind: StructureKind,
    /// Source location when available; unavailable provenance cannot pass fidelity.
    pub span: Option<ByteSpan>,
    /// Literal source content or immutable asset.
    pub content: Content,
}
/// One extracted unit and its exact output correspondence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappedUnit {
    /// Claimed source unit, compared with the independent source measurement.
    pub source: SourceUnit,
    /// Literal Markdown content range; assets instead require retained asset refs.
    pub output: Option<ByteSpan>,
}
/// Source inventory for a single structure cohort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    /// Cohort measured, including known zero counts.
    pub kind: StructureKind,
    /// Ordered independent source inventory.
    pub source: Measured<Vec<SourceUnit>>,
}
/// Ancillary extraction warning. Loss of source evidence cannot be warn-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Warning {
    /// Governing approved rule ID; absent or malformed IDs hold admission.
    pub rule: Option<String>,
    /// Warning retained visibly, including on held documents.
    pub message: String,
}
/// One separately identified extraction attempt; it never replaces its source.
/// Adapters supply independent source measurements, not measurements of output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    /// Unique attempt, including every explicit re-extraction.
    pub attempt: Handle,
    /// N12 immutable capture linkage.
    pub capture: Handle,
    /// Exact immutable source artifact from the capture envelope.
    pub source: Digest,
    /// Original artifact length for source-span validation.
    pub source_length: u64,
    /// Explicit selected extraction method, never an implicit converter chain.
    pub method: String,
    /// Exact qualified profile.
    pub profile: Ref,
    /// Installed provisioned tool identity.
    pub tool: Ref,
    /// Provisioned model identities; an empty set means no model used.
    pub models: Vec<Ref>,
    /// Frozen N57 whole-set processing identities, never executable snippets.
    pub processing: Processing,
    /// Faithful extracted Markdown.
    pub markdown: String,
    /// Typed structural blocks, rows, cells and content correspondences.
    pub units: Vec<MappedUnit>,
    /// Source inventories, including explicit unknown and known zero.
    pub measurements: Vec<Measurement>,
    /// Profile-required source measurement cohorts.
    pub required: Vec<StructureKind>,
    /// Retained immutable original asset identities.
    pub assets: Vec<Ref>,
    /// Ancillary visible warnings.
    pub warnings: Vec<Warning>,
    /// Missing-content indicators; any indicator holds.
    pub missing: Vec<String>,
}
