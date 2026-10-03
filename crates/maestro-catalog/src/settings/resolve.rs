//! Typed restrictive resolution over four preference layers; storage and parsing stay in S1.

pub(crate) use super::standards::{additive, numeric_narrows};
use super::{
    standards::{bounded, constrained},
    types::{Layer, ResolveDiagnostic, ResolvedSettings, ResolvedValue, diagnostic, first},
};

use maestro_settings::{
    LayerName, Registry, Resolved, ResolvedSetting, SettingClass, Source, Value,
};
use std::collections::BTreeMap;

/// Resolves all S1 settings without using storage, parsing, or UI adapters.
#[must_use]
pub fn resolve(registry: &Registry, layers: &Resolved<'_>) -> ResolvedSettings {
    resolve_with_standards(registry, layers, &BTreeMap::new())
}

/// Resolves through C17 with admitted standards above every preference layer.
#[must_use]
pub(crate) fn resolve_with_standards(
    registry: &Registry,
    layers: &Resolved<'_>,
    standards: &BTreeMap<String, Vec<Value>>,
) -> ResolvedSettings {
    let mut result = ResolvedSettings::default();
    for descriptor in registry.descriptors() {
        let key = descriptor.key.as_ref();
        let candidates = layers.get(key).map(candidates).unwrap_or_default();
        let default = registry.default_of(key).cloned();
        let resolved = if descriptor.standard_only && !candidates.is_empty() {
            Err(diagnostic(
                key,
                "standard-only setting cannot be declared locally",
            ))
        } else if let Some(values) = standards.get(key).and_then(|values| values.split_first()) {
            constrained(descriptor, default, values.0, values.1, candidates)
        } else {
            match (descriptor.class, default) {
                (SettingClass::Locked, Some(value)) => locked(key, value, &candidates),
                (SettingClass::Bounded, Some(default)) => bounded(
                    key,
                    &descriptor.kind,
                    default,
                    candidates,
                    &mut result.diagnostics,
                ),
                (SettingClass::Additive, Some(default)) => additive(key, default, candidates),
                (SettingClass::Free, Some(default)) => Ok(first(default, candidates)),
                (_, None) => Err(diagnostic(key, "setting has no built-in default")),
            }
        };
        result.values.insert(key.to_owned(), resolved);
    }
    result
}

/// Recovers supplied values from S1 resolution without rereading settings.
fn candidates(setting: &ResolvedSetting<'_>) -> Vec<(Layer, Value)> {
    let mut result = Vec::new();
    if let Some(source) = source_layer(&setting.source) {
        result.push((source, setting.value.clone()));
    }
    result.extend(
        setting
            .overridden
            .iter()
            .filter_map(|(source, value)| source_layer(source).map(|layer| (layer, value.clone()))),
    );
    result
}

/// Maps S1 provenance to a typed restriction layer.
fn source_layer(source: &Source) -> Option<Layer> {
    match source {
        Source::Flag => Some(Layer::Flag),
        Source::File {
            layer: LayerName::Project,
            ..
        } => Some(Layer::Workspace),
        Source::File {
            layer: LayerName::User,
            ..
        } => Some(Layer::User),
        Source::Default => None,
    }
}

/// Refuses any mutation of a locked value.
fn locked(
    key: &str,
    default: Value,
    candidates: &[(Layer, Value)],
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if candidates.is_empty() {
        return Ok(ResolvedValue {
            value: default,
            source: Layer::Default,
            overridden: Vec::new(),
        });
    }
    Err(diagnostic(
        key,
        "locked setting cannot be changed by a preference layer",
    ))
}
