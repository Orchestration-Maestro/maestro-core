//! Shared strict schema owned by the registered bootstrap inventory kind.
use super::types::{Resource, Value, is_name};
use serde::Deserialize;

/// Exact requirements and file mappings, decoded by the registered kind alone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inventory {
    /// File-stem identity.
    pub(crate) name: String,
    /// Inert binding requirements.
    pub(crate) bindings: Vec<String>,
    /// Executable names, never commands.
    pub(crate) tools: Vec<String>,
    /// Explicit source-to-output mappings.
    pub(crate) files: Vec<InventoryFile>,
}

/// One owner-local source, destination and digest.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InventoryFile {
    /// Relative to this inventory's files directory.
    pub(crate) source: String,
    /// Project-relative output.
    pub(crate) output: String,
    /// Digest verified during composition.
    pub(crate) sha256: String,
}

impl Inventory {
    /// Decode the checked kind fields without reopening or parsing source TOML.
    pub(crate) fn decoded(resource: &Resource) -> Result<Self, String> {
        Value::Table(resource.fields.clone()).decode()
    }
}

/// Two functional names, never a catalog path.
pub(crate) fn inventory_selector(selector: &str) -> Result<(&str, &str), String> {
    selector
        .split_once('/')
        .filter(|(area, inventory)| is_name(area) && is_name(inventory))
        .ok_or_else(|| format!("expected area/inventory names: {selector}"))
}

/// A portable project/source path without escaping components.
pub(crate) fn validate_source_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains(['\\', ':'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(format!("unsafe preset file path: {path}"));
    }
    Ok(())
}

/// Executable names must not include arguments or paths.
pub(crate) fn validate_top_level_name(name: &str) -> Result<(), String> {
    validate_source_path(name)?;
    if name.contains('/') || name.chars().any(char::is_whitespace) {
        return Err(format!("expected a single top-level name: {name}"));
    }
    Ok(())
}
