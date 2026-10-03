//! Typed reference admission and dependency-direction classification.

use super::{
    descriptor::Scope,
    load::Loaded,
    placements::{directories, fits},
    registry::Registry,
    types::{Diagnostic, Resource, ResourceId},
};
use std::collections::{BTreeMap, BTreeSet};

/// The problems of `loaded`'s `requires`: each must exist, of a kind its
/// own kind may require.
pub(super) fn references(
    loaded: &Loaded,
    resources: &BTreeMap<ResourceId, &Resource>,
    registry: &Registry,
    global_languages: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    let key = format!("{}requires", loaded.prefix);
    let admitted = registry
        .kind(&loaded.resource.id.kind)
        .map(|registration| registration.descriptor.requires.clone())
        .unwrap_or_default();
    loaded
        .resource
        .metadata
        .requires
        .iter()
        .filter_map(|target| {
            let message = if registry.kind(&target.kind).is_none() {
                format!("names {target}, whose kind is not registered")
            } else if !admitted
                .iter()
                .any(|kind| kind == "*" || *kind == target.kind)
            {
                format!("kind {} may not require {target}", loaded.resource.id.kind)
            } else if let Some(required) = resources.get(target) {
                let from = layer(&loaded.resource, global_languages, registry);
                let to = layer(required, global_languages, registry);
                if ALLOWED_LAYERS.contains(&(from, to)) {
                    return None;
                }
                format!("{from:?} layer may not require {target} ({to:?} layer)")
            } else {
                format!("names {target}, which does not exist")
            };
            Some(Diagnostic::new(&loaded.metadata_path, &key, message))
        })
        .collect()
}

/// Checked placement layers; workflow labels never enter this classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    /// Common, standards and languages required by them.
    Global,
    /// Framework declarations, not optional selection.
    Core,
    /// An optional language.
    Language,
    /// An explicitly selected team package.
    Team,
    /// A selection root, never an area dependency.
    Preset,
}

/// The sole dependency-direction policy, applied only to explicit `requires`.
/// Same-area wiring (including core agent to core instructions) stays allowed;
/// framework cross-area edges require Global. Cycles are checked separately.
const ALLOWED_LAYERS: [(Layer, Layer); 14] = [
    (Layer::Global, Layer::Global),
    (Layer::Core, Layer::Global),
    (Layer::Core, Layer::Core),
    (Layer::Language, Layer::Global),
    (Layer::Language, Layer::Language),
    (Layer::Team, Layer::Global),
    (Layer::Team, Layer::Core),
    (Layer::Team, Layer::Language),
    (Layer::Team, Layer::Team),
    (Layer::Preset, Layer::Global),
    (Layer::Preset, Layer::Core),
    (Layer::Preset, Layer::Language),
    (Layer::Preset, Layer::Team),
    (Layer::Preset, Layer::Preset),
];

/// Classifies the registered kind and scope, including root-support resources in common.
fn layer(resource: &Resource, global_languages: &BTreeSet<String>, registry: &Registry) -> Layer {
    let Some(registration) = registry.kind(&resource.id.kind) else {
        return Layer::Global;
    };
    let descriptor = &registration.descriptor;
    if descriptor.kind == "preset" {
        return Layer::Preset;
    }
    let scope = descriptor
        .scopes
        .iter()
        .zip(directories(descriptor))
        // Empty Common placement is the Global fallback, not every area's prefix.
        .filter(|(_, directory)| !directory.is_empty())
        .find_map(|(scope, directory)| {
            let count = directory.split('/').filter(|part| !part.is_empty()).count();
            let boundary = resource
                .path
                .split('/')
                .take(count)
                .collect::<Vec<_>>()
                .join("/");
            fits(&directory, &boundary).then_some(*scope)
        });
    match scope {
        Some(Scope::Core) => Layer::Core,
        Some(Scope::Team) => Layer::Team,
        Some(Scope::Language) if !global_languages.contains(area_name(resource)) => Layer::Language,
        _ => Layer::Global,
    }
}

/// A resource's qualified namespace, or its area root's name.
fn area_name(resource: &Resource) -> &str {
    resource
        .id
        .namespace
        .as_deref()
        .unwrap_or(&resource.id.name)
}

/// Languages reached explicitly from common/standards join Global as whole areas.
/// Iterative traversal keeps cycles bounded; the normal graph check still refuses them.
pub(super) fn global_languages(
    resources: &BTreeMap<ResourceId, &Resource>,
    registry: &Registry,
) -> BTreeSet<String> {
    let mut languages: BTreeMap<&str, Vec<&Resource>> = BTreeMap::new();
    let mut pending = Vec::new();
    let empty = BTreeSet::new();
    for resource in resources.values() {
        match layer(resource, &empty, registry) {
            Layer::Global => pending.push(*resource),
            Layer::Language => languages
                .entry(area_name(resource))
                .or_default()
                .push(resource),
            _ => {}
        }
    }
    let mut global = BTreeSet::new();
    while let Some(resource) = pending.pop() {
        for target in &resource.metadata.requires {
            let Some(target) = resources.get(target) else {
                continue;
            };
            if layer(target, &empty, registry) == Layer::Language
                && global.insert(area_name(target).to_owned())
            {
                pending.extend(
                    languages
                        .get(area_name(target))
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
        }
    }
    global
}
