//! Source accounting and explicitly typed context dependencies.

use super::types::{Exclusion, MappingContribution, SourceRange};
use crate::artifact::Digest;
use serde::{Deserialize, Serialize};

/// One eligible canonical contribution in original source coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingEntry {
    /// Nonempty original UTF-8 source bytes.
    pub range: SourceRange,
    /// Corresponding canonical mapping.
    pub mapping: MappingContribution,
}

/// Separately versioned source mapping artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingLedger {
    /// Exactly `maestro-unit-mapping/1`.
    pub schema_version: String,
    /// Digest of the original Markdown.
    pub original_markdown_digest: Digest,
    /// Eligible contributions, in source order.
    pub contributions: Vec<MappingEntry>,
    /// All remaining source bytes, with reasons.
    pub exclusions: Vec<Exclusion>,
}
