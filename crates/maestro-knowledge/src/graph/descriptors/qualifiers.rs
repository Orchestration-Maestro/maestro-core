//! Typed source qualifiers in the frozen /1 JSON field order.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Source qualifiers retained without inferred temporal ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorQualifiers {
    /// Original condition names and values in canonical key order.
    pub conditions: BTreeMap<String, String>,
    /// Original half-open source version bounds, or unknown.
    pub version: DescriptorValidity,
    /// Original half-open world-time bounds, or unknown.
    pub world: DescriptorValidity,
}

/// The original descriptor validity wire shape, not a guessed ordering.
/// Unknown fields are refused so the unknown arm cannot swallow bounded fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum DescriptorValidity {
    /// The source supplies no bounds.
    Unknown {
        /// False for an unknown source value.
        known: bool,
    },
    /// Known half-open bounds; an absent bound is open.
    Bounded {
        /// First excluded value, or an open upper bound.
        end: Option<String>,
        /// True for source-supplied bounds.
        known: bool,
        /// First included value, or an open lower bound.
        start: Option<String>,
    },
}
