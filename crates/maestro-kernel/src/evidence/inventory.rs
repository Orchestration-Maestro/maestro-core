//! Exact, bounded S1 search inventories, separate from supporting passages.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Maximum number of exact groups allowed in a complete inventory.
const MAX_GROUPS: usize = 1000;
/// Maximum serialized JSON size of an inventory response.
const MAX_JSON_BYTES: usize = 32_768;

/// An exact document count grouped by set or documented version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Inventory {
    /// The pinned generation's documents, grouped by `metadata.set`.
    DocumentsBySet {
        /// An exact set filter, if the request named one.
        set_filter: Option<String>,
        /// The distinct document count of the selected population.
        total_documents: u64,
        /// Counts by set, null last.
        sets: Vec<InventoryCount>,
    },
    /// The pinned generation's documents, grouped by `metadata.version`.
    Versions {
        /// An exact set filter, if the request named one.
        set_filter: Option<String>,
        /// The distinct document count of the selected population.
        total_documents: u64,
        /// Counts by version, null last.
        versions: Vec<InventoryCount>,
    },
}

/// The count for one exact metadata value, with `None` for an unlabelled row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryCount {
    /// The exact text value, or `None` when the metadata is not text.
    pub value: Option<String>,
    /// The number of distinct documents in this group.
    pub documents: u64,
}

impl Inventory {
    /// Whether the groups agree with the exact total and wire bounds.
    pub(super) fn validate(&self) -> Result<(), String> {
        let (total_documents, groups) = match self {
            Self::DocumentsBySet {
                total_documents,
                sets,
                ..
            } => (total_documents, sets),
            Self::Versions {
                total_documents,
                versions,
                ..
            } => (total_documents, versions),
        };
        if groups.len() > MAX_GROUPS {
            return Err("inventory exceeds 1000 groups".to_owned());
        }
        let mut sum = 0_u64;
        let mut previous: Option<Option<&str>> = None;
        for group in groups {
            if previous.is_some_and(|previous| {
                compare_groups(previous, group.value.as_deref()) != Ordering::Less
            }) {
                return Err(
                    "inventory groups must be distinct and ordered, with null last".to_owned(),
                );
            }
            previous = Some(group.value.as_deref());
            sum = sum
                .checked_add(group.documents)
                .ok_or_else(|| "inventory group total overflows".to_owned())?;
        }
        if sum != *total_documents {
            return Err("inventory group counts do not equal total_documents".to_owned());
        }
        let encoded =
            serde_json::to_vec(self).map_err(|_| "inventory could not be encoded".to_owned())?;
        if encoded.len() > MAX_JSON_BYTES {
            return Err("inventory exceeds 32768 serialized bytes".to_owned());
        }
        Ok(())
    }
}

/// Compare exact group values, sorting null after every text value.
fn compare_groups(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
