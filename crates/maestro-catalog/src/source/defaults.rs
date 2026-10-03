//! Source-only snapshot and backend extraction into the shared S1 producer.
use super::{
    scan::Snapshot,
    tree::{EntryKind, SourceTree},
    types::{Diagnostic, Refusal, Resource, Value},
};
use crate::{limits::Limits, settings::defaults::manifest_registry};
use maestro_settings::{Registry, SCHEMA};
use std::{collections::BTreeMap, io};

/// Exact non-resource placement; the bounded source snapshot owns its bytes.
pub(crate) const DEFAULTS_PATH: &str = "settings/defaults.toml";

/// Read and validate non-resource defaults from the same no-follow catalog snapshot.
/// Returns the checked registry and the validated common input for later selection.
pub(super) fn from_snapshot(
    snapshot: &Snapshot,
    resources: &[Resource],
    registry: Option<&Registry>,
    limits: &Limits,
) -> Result<(Option<Registry>, Option<String>), Refusal> {
    if snapshot.list("settings").is_ok_and(|entries| {
        entries
            .iter()
            .any(|entry| entry.name == "defaults.toml" && entry.kind != EntryKind::File)
    }) {
        return Err(refusal(
            DEFAULTS_PATH,
            "",
            "defaults must be a regular file",
        ));
    }
    let common = match snapshot.read(DEFAULTS_PATH, limits.source_file_bytes) {
        Ok(bytes) => {
            Some(String::from_utf8(bytes).map_err(|_| refusal(DEFAULTS_PATH, "", "not UTF-8"))?)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(Refusal {
                diagnostics: vec![Diagnostic::unreadable(DEFAULTS_PATH, error.to_string())],
            });
        }
    };
    let settings = match registry {
        Some(registry) => Some(from_resources(
            registry,
            resources,
            common.as_deref(),
            limits,
        )?),
        None if common.is_some() => {
            return Err(refusal(
                DEFAULTS_PATH,
                "",
                "defaults require typed descriptors",
            ));
        }
        None => None,
    };
    Ok((settings, common))
}

/// Convert already checked backend fields to S1 inputs without another source read.
pub(crate) fn from_resources(
    registry: &Registry,
    resources: &[Resource],
    common: Option<&str>,
    limits: &Limits,
) -> Result<Registry, Refusal> {
    let mut inputs = Vec::new();
    if let Some(common) = common {
        inputs.push((DEFAULTS_PATH.to_owned(), common.to_owned()));
    }
    for resource in resources
        .iter()
        .filter(|resource| resource.id.kind == "backend" && resource.id.name == "graphdb")
    {
        let mut fields = BTreeMap::from([("schema", Value::Text(SCHEMA.to_owned()))]);
        if let Some(kind) = resource.fields.get("type") {
            fields.insert("graph.engine", kind.clone());
        }
        if let Some(controls) = resource.fields.get("graphdb") {
            fields.insert("graphdb", controls.clone());
        }
        let text = toml::to_string(&fields)
            .map_err(|error| refusal(&resource.path, "", error.to_string()))?;
        inputs.push((resource.path.clone(), text));
    }
    manifest_registry(registry, &inputs, limits)
        .map_err(|(path, key, message)| refusal(&path, &key, message))
}

/// Locate registry and producer errors at their manifest input.
fn refusal(path: &str, key: &str, message: impl Into<String>) -> Refusal {
    Refusal {
        diagnostics: vec![Diagnostic::new(path, key, message)],
    }
}
