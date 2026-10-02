//! The checker: discovery, each resource read by its kind, then the checks
//! across resources: duplicate IDs, references and their target kinds, each
//! kind's catalog rules, dependency cycles and declared closures. A refusal
//! lists at most [`DIAGNOSTICS`] diagnostics, then a count of the rest.

use super::standards::check_references;
use super::{
    area_walk,
    defaults::{DEFAULTS_PATH, from_snapshot},
    descriptor::Layout,
    graph,
    load::{Context, Loaded, load},
    ownership::{GENERATED_CODEOWNERS, area_path},
    references::{global_languages, references},
    registry::Registry,
    scan::{Snapshot, scan},
    tree::{Directory, SourceTree},
    types::{Catalog, Diagnostic, Known, Maturity, Refusal, Resource, ResourceId},
    walk::walk_snapshot,
};
use crate::limits::Limits;
use std::{
    collections::{BTreeMap, BTreeSet},
    io, str,
};

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
    let (catalog, generated) = build(tree, registry, limits, known)?;
    catalog.verify_generated(generated.as_deref())?;
    Ok(catalog)
}

/// Build a catalog and retain the bounded generated-file bytes from the same snapshot.
/// File safety always runs; renderers may regenerate stale bytes. Ordinary checking
/// must also call [`Catalog::verify_generated`] before reporting success.
///
/// # Errors
/// Source, placement, ownership, size or no-follow regular-file validation refuses.
pub fn build(
    tree: &dyn SourceTree,
    registry: &Registry,
    limits: &Limits,
    known: Known<'_>,
) -> Result<(Catalog, Option<Vec<u8>>), Refusal> {
    let snapshot = scan(tree, limits)?;
    build_snapshot(&snapshot, registry, limits, known)
}

impl Directory {
    /// Check and retain the one bounded snapshot used to decode this directory.
    pub(crate) fn checked_snapshot(
        &self,
        registry: &Registry,
        limits: &Limits,
        known: Known<'_>,
    ) -> Result<(Catalog, Snapshot), Refusal> {
        let snapshot = scan(self, limits)?;
        let (catalog, generated) = build_snapshot(&snapshot, registry, limits, known)?;
        catalog.verify_generated(generated.as_deref())?;
        Ok((catalog, snapshot))
    }
}

/// Load registered resources and then verify their exact static/dynamic claims.
fn build_snapshot(
    snapshot: &Snapshot,
    registry: &Registry,
    limits: &Limits,
    known: Known<'_>,
) -> Result<(Catalog, Option<Vec<u8>>), Refusal> {
    let found = walk_snapshot(snapshot, registry, limits)?;
    let mut diagnostics = found.diagnostics;
    let context = Context {
        tree: snapshot,
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
    let mut claimed = found.claimed;
    for resource in &loaded {
        for path in &resource.resource.data {
            claimed.insert(path.clone());
            if let Err(error) = snapshot.read(path, limits.source_file_bytes) {
                diagnostics.push(Diagnostic::unreadable(
                    path,
                    format!("cannot read inventoried asset: {error}"),
                ));
            }
        }
    }
    let resources: Vec<_> = loaded
        .iter()
        .map(|loaded| loaded.resource.clone())
        .collect();
    let (settings, common) = from_snapshot(snapshot, &resources, known.settings.registry(), limits)
        .unwrap_or_else(|refusal| {
            diagnostics.extend(refusal.diagnostics);
            (None, None)
        });
    if common.is_some() {
        claimed.insert(DEFAULTS_PATH.to_owned());
    }
    diagnostics.extend(area_walk::unclaimed(snapshot, &claimed));
    if diagnostics.is_empty() {
        let known = Known {
            settings: settings
                .as_ref()
                .map_or(known.settings, |settings| settings),
            ..known
        };
        if let Err(refusal) = super::backend_extensions::from_snapshot(
            snapshot,
            &mut loaded,
            known.settings.registry(),
            limits,
        ) {
            diagnostics.extend(refusal.diagnostics);
        }
        diagnostics.extend(across(&loaded, registry, known));
    }
    if diagnostics.is_empty() {
        let mut resources: Vec<Resource> =
            loaded.into_iter().map(|loaded| loaded.resource).collect();
        resources.sort_by(|left, right| left.id.cmp(&right.id));
        let catalog = Catalog {
            resources,
            common_defaults: common,
        };
        let generated = match snapshot.read(GENERATED_CODEOWNERS.path, limits.source_file_bytes) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(Refusal {
                    diagnostics: vec![Diagnostic::unreadable(
                        GENERATED_CODEOWNERS.path,
                        format!("cannot read: {error}"),
                    )],
                });
            }
        };
        return Ok((catalog, generated));
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
impl Catalog {
    /// Verify optional generated bytes retained by [`build`], without rereading/writing.
    /// An absent output is allowed; the CLI's explicit `--check` requires its presence.
    ///
    /// # Errors
    /// A diagnostic naming the generated path and the first differing rule.
    pub fn verify_generated(&self, bytes: Option<&[u8]>) -> Result<(), Refusal> {
        let Some(bytes) = bytes else {
            return Ok(());
        };
        let result = str::from_utf8(bytes)
            .map_err(|error| error.to_string())
            .and_then(|text| (GENERATED_CODEOWNERS.validate)(self, text));
        result.map_err(|message| Refusal {
            diagnostics: vec![Diagnostic::new(GENERATED_CODEOWNERS.path, "", message)],
        })
    }
}
/// The problems across the resources `loaded`.
fn across(loaded: &[Loaded], registry: &Registry, known: Known<'_>) -> Vec<Diagnostic> {
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
    diagnostics.extend(super::standards::check(
        &resources.values().copied().collect::<Vec<_>>(),
        known,
    ));
    let area_paths: BTreeSet<&str> = resources
        .values()
        .filter(|resource| {
            registry
                .kind(&resource.id.kind)
                .is_some_and(|registration| {
                    matches!(registration.descriptor.layout, Layout::Area { .. })
                })
        })
        .map(|resource| resource.path.as_str())
        .collect();
    for loaded in catalog.values() {
        let resource = &loaded.resource;
        let Some(registration) = registry.kind(&resource.id.kind) else {
            continue;
        };
        let path = area_path(&resource.path, &registration.descriptor);
        if !area_paths.contains(path.as_str()) {
            diagnostics.push(Diagnostic::new(
                &resource.path,
                "ownership",
                format!("missing area descriptor {path}"),
            ));
        }
    }
    let global_languages = global_languages(&resources, registry);
    for resource in catalog.values() {
        diagnostics.extend(references(
            resource,
            &resources,
            registry,
            &global_languages,
        ));
        rules(resource, &resources, registry, &mut diagnostics);
    }
    let edges = edges(&catalog, registry);
    let components = graph::components(&edges);
    let loaded: Vec<&Loaded> = catalog.into_values().collect();
    diagnostics.extend(cycles(&loaded, &edges, &components));
    diagnostics.extend(closures(&loaded, &edges, &components, registry));
    diagnostics
}

/// Notes the problems the kind's own catalog rules find in `loaded`.
fn rules(
    loaded: &Loaded,
    resources: &BTreeMap<ResourceId, &Resource>,
    registry: &Registry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut problems = Vec::new();
    check_references(&loaded.resource, resources, &mut problems);
    if let Some(rules) = registry
        .kind(&loaded.resource.id.kind)
        .and_then(|registration| registration.rules)
    {
        rules.check_catalog(&loaded.resource, resources, &mut problems);
    }
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
    let mandatory: Vec<ResourceId> = catalog
        .keys()
        .filter(|id| {
            id.kind == "standard"
                || (id.kind == "package" && ["common", "core"].contains(&id.name.as_str()))
        })
        .cloned()
        .collect();
    catalog
        .iter()
        .map(|(id, loaded)| {
            let mut targets: Vec<ResourceId> = loaded.resource.metadata.requires.clone();
            if id.kind == "preset" {
                targets.extend(mandatory.iter().cloned());
            }
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
