//! Exact natural-language grammar for S1 document and version inventories.

use crate::{
    lexical,
    query::{
        INVENTORY_DOCUMENT_FORMS, INVENTORY_FILTERS, INVENTORY_VERSION_FORMS, QueryKind, Understood,
    },
};
use maestro_kernel::{retrieval::InventoryRequest, retrieval::normalize_whitespace};
use std::{error, fmt};

/// Why an inventory-like Global query has malformed exact grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InventoryParseError;

impl fmt::Display for InventoryParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("inventory set must be one nonempty JSON string")
    }
}

impl error::Error for InventoryParseError {}

/// Parses only the six supported complete inventory forms.
pub(crate) fn inventory_request(
    understood: &Understood,
) -> Result<Option<InventoryRequest>, InventoryParseError> {
    if understood.kind != QueryKind::Global {
        return Ok(None);
    }
    let normalized = normalize_whitespace(&understood.normalized);
    let query = normalized.strip_suffix('?').unwrap_or(&normalized);
    let folded = fold(query);
    for (forms, versions) in [
        (INVENTORY_DOCUMENT_FORMS, false),
        (INVENTORY_VERSION_FORMS, true),
    ] {
        for form in forms {
            let form = fold(form);
            if folded == form {
                return Ok(Some(request(versions, None)));
            }
            let prefix = INVENTORY_FILTERS
                .iter()
                .map(|suffix| format!("{form}{suffix}"))
                .find(|prefix| folded == *prefix || folded.starts_with(&format!("{prefix} ")));
            if let Some(prefix) = prefix {
                return parse_filtered(query, &prefix, versions).map(Some);
            }
        }
    }
    Ok(None)
}

/// Parses the single JSON string after one supported exact filter prefix.
fn parse_filtered(
    query: &str,
    folded_prefix: &str,
    versions: bool,
) -> Result<InventoryRequest, InventoryParseError> {
    for (offset, _) in query.char_indices() {
        if fold(query.get(..offset).unwrap_or_default().trim_end()) != folded_prefix {
            continue;
        }
        let set = serde_json::from_str::<String>(query.get(offset..).unwrap_or_default())
            .map_err(|_| InventoryParseError)?;
        if set.is_empty() {
            return Err(InventoryParseError);
        }
        return Ok(request(versions, Some(set)));
    }
    Err(InventoryParseError)
}

/// Builds the matching kernel request.
fn request(versions: bool, set: Option<String>) -> InventoryRequest {
    if versions {
        InventoryRequest::Versions { set }
    } else {
        InventoryRequest::DocumentsBySet { set }
    }
}

/// Folds keywords without changing the exact JSON filter value.
fn fold(text: &str) -> String {
    lexical::fold(text).to_lowercase()
}
