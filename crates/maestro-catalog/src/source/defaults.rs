//! Manifest producers replace declared defaults in the sole S1 lowest slot.

use super::{
    scan::Snapshot,
    tree::{EntryKind, SourceTree},
    types::{Diagnostic, Refusal, Resource, Value as SourceValue},
};
use crate::limits::Limits;
use maestro_settings::{Layer, MAX_FILE_BYTES, Registry, SCHEMA, Value};
use std::{borrow::Cow, collections::BTreeMap, io};

/// Exact non-resource placement; the source snapshot owns its bytes.
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
        Some(registry) => Some(manifest_registry(
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

/// Construct the S1 registry with one producer for each manifest-declared default.
/// Undeclared keys retain registered defaults; locked defaults cannot be authored.
/// The complete common file and every supplied backend control are checked before
/// any preference can mask them. No engine is opened and no secret is resolved.
///
/// # Errors
/// Refuses invalid/locked values, unknown keys and repeated producers, even equal ones.
pub(crate) fn manifest_registry(
    registry: &Registry,
    resources: &[Resource],
    common: Option<&str>,
    limits: &Limits,
) -> Result<Registry, Refusal> {
    let mut producers = BTreeMap::new();
    if let Some(text) = common {
        produce(&mut producers, registry, DEFAULTS_PATH, text, limits)?;
    }
    for resource in resources
        .iter()
        .filter(|resource| resource.id.kind == "backend" && resource.id.name == "graphdb")
    {
        let mut fields = BTreeMap::from([("schema", SourceValue::Text(SCHEMA.to_owned()))]);
        if let Some(kind) = resource.fields.get("type") {
            fields.insert("graph.engine", kind.clone());
        }
        if let Some(controls) = resource.fields.get("graphdb") {
            fields.insert("graphdb", controls.clone());
        }
        let text = toml::to_string(&fields)
            .map_err(|error| refusal(&resource.path, "", error.to_string()))?;
        produce(&mut producers, registry, &resource.path, &text, limits)?;
    }
    let descriptors: Vec<_> = registry
        .descriptors()
        .map(|descriptor| {
            let mut descriptor = descriptor.clone();
            if let Some((_, value)) = producers.get(descriptor.key.as_ref()) {
                descriptor.default = Cow::Owned(value.to_string());
            }
            descriptor
        })
        .collect();
    Registry::new(&descriptors).map_err(|error| refusal(DEFAULTS_PATH, &error.key, &error.reason))
}

/// Validate whole inputs with S1, then record each producer once, even equal values.
fn produce(
    producers: &mut BTreeMap<String, (String, Value)>,
    registry: &Registry,
    path: &str,
    text: &str,
    limits: &Limits,
) -> Result<(), Refusal> {
    let layer = Layer::parse_preferences(
        registry,
        text,
        limits.source_file_bytes.min(MAX_FILE_BYTES as u64),
        limits.source_depth,
    )
    .map_err(|error| refusal(path, "", error.to_string()))?;
    for (key, value) in layer.iter() {
        if let Some((first, _)) = producers.get(key) {
            return Err(refusal(
                path,
                key,
                format!("duplicate default producer; also {first}"),
            ));
        }
        producers.insert(key.to_owned(), (path.to_owned(), value.clone()));
    }
    Ok(())
}

/// Locate registry and producer errors at their manifest input.
fn refusal(path: &str, key: &str, message: impl Into<String>) -> Refusal {
    Refusal {
        diagnostics: vec![Diagnostic::new(path, key, message)],
    }
}
