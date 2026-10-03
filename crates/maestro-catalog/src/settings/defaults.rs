//! One redacting S1 default producer, shared by source checking and sessions.
use crate::limits::Limits;
use maestro_settings::{Layer, LayerError, MAX_FILE_BYTES, Registry, SCHEMA, SettingClass, Value};
use std::{borrow::Cow, collections::BTreeMap, fmt::Write as _};

/// Construct the sole lowest S1 slot from exact named preference documents.
/// Validate all inputs before any preference can mask them, without resolving secrets.
///
/// # Errors
/// Redacted input path, setting key and reason for invalid/locked or repeated values.
pub(crate) fn manifest_registry(
    registry: &Registry,
    inputs: &[(String, String)],
    limits: &Limits,
) -> Result<Registry, (String, String, String)> {
    let mut producers = BTreeMap::new();
    for (path, text) in inputs {
        produce(&mut producers, registry, path, text, limits)?;
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
    Registry::new(&descriptors).map_err(|error| ("defaults".to_owned(), error.key, error.reason))
}

/// Validate whole inputs with S1, then record each producer once, even equal values.
fn produce(
    producers: &mut BTreeMap<String, (String, Value)>,
    registry: &Registry,
    path: &str,
    text: &str,
    limits: &Limits,
) -> Result<(), (String, String, String)> {
    let layer = Layer::parse_preferences(
        registry,
        text,
        limits.source_file_bytes.min(MAX_FILE_BYTES as u64),
        limits.source_depth,
    )
    .map_err(|error| (path.to_owned(), String::new(), redacted(error)))?;
    for (key, value) in layer.iter() {
        if let Some((first, _)) = producers.get(key) {
            return Err((
                path.to_owned(),
                key.to_owned(),
                format!("duplicate default producer; also {first}"),
            ));
        }
        producers.insert(key.to_owned(), (path.to_owned(), value.clone()));
    }
    Ok(())
}

/// A common defaults refusal must never quote the refused value.
fn redacted(error: LayerError) -> String {
    match error {
        LayerError::Refused { key, reason, .. } => format!("{key}: {reason}"),
        LayerError::NotToml(_) => "invalid TOML in manifest defaults".to_owned(),
        other => other.to_string(),
    }
}

/// Persist the admitted lowest slot, not resolved user/workspace/flag values.
pub(crate) fn frozen(registry: &Registry) -> String {
    let mut text = format!("schema = {SCHEMA:?}\n");
    for descriptor in registry
        .descriptors()
        .filter(|descriptor| descriptor.class != SettingClass::Locked)
    {
        if let Some(value) = registry.default_of(&descriptor.key) {
            let _ = writeln!(text, "{:?} = {}", descriptor.key, value.to_toml());
        }
    }
    text
}
