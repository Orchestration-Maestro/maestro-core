//! The checker: discovery, each resource read by its kind, then the checks
//! across resources: duplicate IDs, references and their target kinds, each
//! kind's catalog rules, dependency cycles and declared closures. A refusal
//! lists at most [`DIAGNOSTICS`] diagnostics, then a count of the rest.

use super::{
    graph,
    load::{Context, Loaded, load},
    registry::Registry,
    scan::scan,
    tree::SourceTree,
    types::{Catalog, Diagnostic, Known, Maturity, Refusal, Resource, ResourceId},
    walk::walk_snapshot,
};
use crate::limits::Limits;
use std::collections::BTreeMap;

/// The most diagnostics a refusal lists; one more counts the rest.
const DIAGNOSTICS: usize = 1_000;

/// Checks the catalog `tree` holds against the kinds of `registry`, under
/// `limits`, with `known` as what the checker knows beyond the catalog.
/// Checking reads files as data: it runs no template or script, only the
/// reviewed hooks the kinds select.
///
/// # Errors
///
/// A [`Refusal`] with every diagnostic found. The checks across resources
/// run only once every file passes its own.
pub fn check(
    tree: &dyn SourceTree,
    registry: &Registry,
    limits: &Limits,
    known: Known<'_>,
) -> Result<Catalog, Refusal> {
    let snapshot = scan(tree, limits)?;
    let found = walk_snapshot(&snapshot, registry, limits)?;
    let mut diagnostics = found.diagnostics;
    let context = Context {
        tree: &snapshot,
        limits,
        known,
    };
    let mut loaded = Vec::new();
    for unit in &found.units {
        let Some(registration) = registry.kind(&unit.kind) else {
            continue;
        };
        match load(unit, registration, context) {
            Ok(resource) => loaded.push(resource),
            Err(found) => diagnostics.extend(found),
        }
    }
    if diagnostics.is_empty() {
        diagnostics = across(&loaded, registry);
    }
    if diagnostics.is_empty() {
        let mut resources: Vec<Resource> =
            loaded.into_iter().map(|loaded| loaded.resource).collect();
        resources.sort_by(|left, right| left.id.cmp(&right.id));
        return Ok(Catalog { resources });
    }
    diagnostics.sort();
    diagnostics.dedup();
    if diagnostics.len() > DIAGNOSTICS {
        let hidden = diagnostics.split_off(DIAGNOSTICS);
        let mut rest = Diagnostic::new(
            "",
            "",
            format!("{} more diagnostics not shown", hidden.len()),
        );
        rest.cause = hidden
            .iter()
            .map(|hidden| hidden.cause)
            .fold(rest.cause, Ord::max);
        diagnostics.push(rest);
    }
    Err(Refusal { diagnostics })
}

/// The problems across the resources `loaded`.
fn across(loaded: &[Loaded], registry: &Registry) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut catalog: BTreeMap<ResourceId, &Loaded> = BTreeMap::new();
    for resource in loaded {
        if let Some(first) = catalog.get(&resource.resource.id) {
            diagnostics.push(Diagnostic::new(
                &resource.resource.path,
                "",
                format!(
                    "duplicate ID {}, also {}",
                    resource.resource.id, first.resource.path
                ),
            ));
        } else {
            catalog.insert(resource.resource.id.clone(), resource);
        }
    }
    let resources: BTreeMap<ResourceId, &Resource> = catalog
        .iter()
        .map(|(id, loaded)| (id.clone(), &loaded.resource))
        .collect();
    for resource in catalog.values() {
        diagnostics.extend(references(resource, &resources, registry));
        rules(resource, &resources, registry, &mut diagnostics);
    }
    let edges = edges(&catalog, registry);
    let components = graph::components(&edges);
    let loaded: Vec<&Loaded> = catalog.into_values().collect();
    diagnostics.extend(cycles(&loaded, &edges, &components));
    diagnostics.extend(closures(&loaded, &edges, &components, registry));
    diagnostics
}

/// The problems of `loaded`'s `requires`: each must exist, of a kind its
/// own kind may require.
fn references(
    loaded: &Loaded,
    resources: &BTreeMap<ResourceId, &Resource>,
    registry: &Registry,
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
            } else if !resources.contains_key(target) {
                format!("names {target}, which does not exist")
            } else {
                return None;
            };
            Some(Diagnostic::new(&loaded.metadata_path, &key, message))
        })
        .collect()
}

/// Notes the problems the kind's own catalog rules find in `loaded`.
fn rules(
    loaded: &Loaded,
    resources: &BTreeMap<ResourceId, &Resource>,
    registry: &Registry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(rules) = registry
        .kind(&loaded.resource.id.kind)
        .and_then(|registration| registration.rules)
    else {
        return;
    };
    let mut problems = Vec::new();
    rules.check_catalog(&loaded.resource, resources, &mut problems);
    diagnostics.extend(
        problems
            .into_iter()
            .map(|(key, message)| Diagnostic::new(&loaded.resource.path, key, message)),
    );
}

/// Each resource's dependencies, its nodes numbered in ID order: its
/// `requires` and its kind's own edges, only those that exist, sorted.
fn edges(catalog: &BTreeMap<ResourceId, &Loaded>, registry: &Registry) -> Vec<Vec<usize>> {
    let position: BTreeMap<&ResourceId, usize> = catalog
        .keys()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect();
    catalog
        .iter()
        .map(|(id, loaded)| {
            let mut targets: Vec<ResourceId> = loaded.resource.metadata.requires.clone();
            if let Some(rules) = registry
                .kind(&id.kind)
                .and_then(|registration| registration.rules)
            {
                targets.extend(rules.edges(&loaded.resource));
            }
            let mut targets: Vec<usize> = targets
                .iter()
                .filter_map(|target| position.get(target).copied())
                .collect();
            targets.sort_unstable();
            targets.dedup();
            targets
        })
        .collect()
}

/// The members of a cycle a diagnostic lists before counting the rest.
const LISTED_MEMBERS: usize = 10;

/// One diagnostic per dependency cycle, at its first member's `requires`,
/// listing its first members.
fn cycles(loaded: &[&Loaded], edges: &[Vec<usize>], components: &[Vec<usize>]) -> Vec<Diagnostic> {
    components
        .iter()
        .filter(|component| graph::is_cycle(component, edges))
        .filter_map(|component| {
            let first = loaded.get(*component.first()?)?;
            let names: Vec<String> = component
                .iter()
                .take(LISTED_MEMBERS)
                .filter_map(|member| loaded.get(*member))
                .map(|member| member.resource.id.to_string())
                .collect();
            let rest = component.len().saturating_sub(LISTED_MEMBERS);
            let more = if rest == 0 {
                String::new()
            } else {
                format!(" and {rest} more")
            };
            Some(Diagnostic::new(
                &first.metadata_path,
                format!("{}requires", first.prefix),
                format!("dependency cycle among {}{more}", names.join(", ")),
            ))
        })
        .collect()
}

/// The problems of each declared closure a reviewed root roots: every
/// member it reaches must be `reviewed`; the first that is not is noted.
fn closures(
    loaded: &[&Loaded],
    edges: &[Vec<usize>],
    components: &[Vec<usize>],
    registry: &Registry,
) -> Vec<Diagnostic> {
    let unreviewed: Vec<bool> = loaded
        .iter()
        .map(|member| member.resource.metadata.maturity != Maturity::Reviewed)
        .collect();
    let first = graph::first_flagged(edges, components, &unreviewed);
    loaded
        .iter()
        .zip(first)
        .filter(|(root, _)| {
            root.resource.metadata.maturity == Maturity::Reviewed
                && registry
                    .kind(&root.resource.id.kind)
                    .is_some_and(|registration| registration.descriptor.closure_root)
        })
        .filter_map(|(root, member)| {
            let member = loaded.get(member?)?;
            Some(Diagnostic::new(
                &root.metadata_path,
                format!("{}requires", root.prefix),
                format!(
                    "closure member {} is {}; a closure admits only reviewed members",
                    member.resource.id,
                    member.resource.metadata.maturity.as_str()
                ),
            ))
        })
        .collect()
}
