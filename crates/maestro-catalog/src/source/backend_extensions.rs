//! Owner-local, inert backend additions and descriptor-checked ceiling minima.

use super::{
    backends::BACKENDS,
    defaults::from_resources,
    descriptor::{Field, FieldType},
    load::Loaded,
    parse::{fields, is_name, toml_table},
    registry::Registry as CatalogRegistry,
    scan::Snapshot,
    tree::SourceTree,
    types::{Catalog, Diagnostic, Problems, Refusal, Resource, ResourceId, Value},
};
use crate::limits::Limits;
use maestro_settings::{Registry, SettingClass, Value as SettingValue};
use std::collections::{BTreeMap, BTreeSet};

/// Checker-owned decoded input; not an authorable field of the package descriptor.
const INPUTS: &str = "checked-backend-extensions";

/// The derived logical view. Names never create stores or authorize effects.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BackendExtensions {
    /// Canonical S1 graph control keys with the minimum selected ceiling.
    pub ceilings: BTreeMap<String, i64>,
    /// Owner-qualified logical graph projection names.
    pub projections: BTreeSet<String>,
    /// Owner-qualified logical vector collection names, not physical identities.
    pub collections: BTreeSet<String>,
}

impl Catalog {
    /// Derive the selected logical additions and minimum graph ceilings.
    /// Removing a package removes only its derived declarations, never its data.
    /// This view neither creates stores nor grants tool or filesystem authority.
    ///
    /// # Errors
    /// Selection, type compatibility, namespace, collision or widening refusals.
    pub fn effective_backend_extensions(
        &self,
        selected: &[ResourceId],
        registry: &CatalogRegistry,
    ) -> Result<BackendExtensions, Refusal> {
        let members = self.selection(selected, registry)?;
        self::selected(&members, self.common_defaults.as_deref())
    }
}

/// Validate exact role selectors, with no arbitrary paths or repeated roles.
pub(super) fn selectors(resource: &Resource) -> Result<Vec<String>, String> {
    let Some(value) = resource.fields.get("backend_extensions") else {
        return Ok(Vec::new());
    };
    let roles = value
        .decode::<Vec<String>>()
        .map_err(|_| "expected backend role list")?;
    if resource.id.kind != "package" || ["common", "core"].contains(&resource.id.name.as_str()) {
        return Err("backend extensions belong only to optional packages".to_owned());
    }
    let mut seen = BTreeSet::new();
    for role in &roles {
        if !BACKENDS.iter().any(|backend| backend.role == role) {
            return Err("unknown backend extension role".to_owned());
        }
        if !seen.insert(role) {
            return Err("duplicate backend extension role".to_owned());
        }
    }
    Ok(roles)
}

/// Exact source files claimed by package selectors; missing/unlisted files refuse.
pub(super) fn assets(resource: &Resource) -> Result<Vec<String>, String> {
    let parent = resource.path.strip_suffix("package.toml").unwrap_or("");
    selectors(resource).map(|roles| {
        roles
            .into_iter()
            .map(|role| format!("{parent}backends/{role}/config.toml"))
            .collect()
    })
}

/// Registered current extension fields: no base/type/endpoint/trust replacement.
fn shape(role: &str) -> Vec<Field> {
    match role {
        "graphdb" => vec![
            Field::optional("projections", FieldType::TextSequence),
            Field::optional(
                "graphdb",
                FieldType::Delegated {
                    validator: "backend-extension".to_owned(),
                },
            ),
        ],
        "vectordb" => vec![Field::optional("collections", FieldType::TextSequence)],
        // C47b adds the checked server/tool schema; no unchecked records today.
        "mcp" => vec![Field::optional("mcp", FieldType::Table { fields: vec![] })],
        _ => vec![],
    }
}

/// Decode exact inputs from the held bounded snapshot, retaining their checked trees.
/// Called after the unique manifest default producer has been validated.
pub(super) fn from_snapshot(
    snapshot: &Snapshot,
    loaded: &mut [Loaded],
    registry: Option<&Registry>,
    limits: &Limits,
) -> Result<(), Refusal> {
    let fallback = Registry::built_in().map_err(|error| refusal("", "", error.to_string()))?;
    let registry = registry.unwrap_or(&fallback);
    for loaded in loaded.iter_mut() {
        let resource = &mut loaded.resource;
        let roles = selectors(resource)
            .map_err(|message| refusal(&resource.path, "backend_extensions", message))?;
        let mut inputs = BTreeMap::new();
        for role in roles {
            let parent = resource.path.strip_suffix("package.toml").unwrap_or("");
            let path = format!("{parent}backends/{role}/config.toml");
            let bytes = snapshot
                .read(&path, limits.source_file_bytes)
                .map_err(|error| refusal(&path, "", error.to_string()))?;
            let text = String::from_utf8(bytes).map_err(|_| refusal(&path, "", "not UTF-8"))?;
            let table = toml_table(&text, limits)
                .map_err(|(key, message)| refusal(&path, &key, message))?;
            inputs.insert(role, Value::Table(table));
        }
        if !inputs.is_empty() {
            resource
                .fields
                .insert(INPUTS.to_owned(), Value::Table(inputs));
        }
    }
    effective(
        &loaded
            .iter()
            .map(|loaded| &loaded.resource)
            .collect::<Vec<_>>(),
        registry,
        limits,
    )
    .map(|_| ())
}

/// Validate the current base type and minimize all selected numeric constraints.
/// Each individual constraint is compared to the base, never to input order.
pub(super) fn effective(
    resources: &[&Resource],
    registry: &Registry,
    limits: &Limits,
) -> Result<BackendExtensions, Refusal> {
    let mut view = BackendExtensions::default();
    for resource in resources {
        let roles = selectors(resource)
            .map_err(|message| refusal(&resource.path, "backend_extensions", message))?;
        for role in roles {
            let parent = resource.path.strip_suffix("package.toml").unwrap_or("");
            let path = format!("{parent}backends/{role}/config.toml");
            let base = resources.iter().find(|base| {
                base.id.kind == "backend"
                    && base.id.name == role
                    && base.id.namespace.as_deref() == Some("core")
            });
            let Some(base) = base else {
                return Err(refusal(&path, "", "missing core backend base"));
            };
            let Some(Value::Table(inputs)) = resource.fields.get(INPUTS) else {
                return Err(refusal(
                    &path,
                    "",
                    "missing checked backend extension input",
                ));
            };
            let Some(Value::Table(table)) = inputs.get(&role) else {
                return Err(refusal(&path, "", "missing checked backend extension role"));
            };
            let descriptor = BACKENDS
                .iter()
                .find(|backend| backend.role == role)
                .ok_or_else(|| refusal(&path, "", "unknown backend extension role"))?;
            let kind = base.fields.get("type").and_then(Value::text);
            if kind.is_none_or(|kind| !descriptor.types.contains(&kind))
                || (kind == Some("none") && !table.is_empty())
            {
                return Err(refusal(
                    &path,
                    "",
                    "extension incompatible with core backend type",
                ));
            }
            let mut problems = Problems::new();
            fields(table, &shape(&role), "", &mut problems);
            if let Some((key, message)) = problems.into_iter().next() {
                return Err(refusal(&path, &key, message));
            }
            if let Some(value) = table.get("graphdb") {
                narrow(value, registry, &path, &mut view)?;
            }
            additions(
                table.get("projections"),
                resource,
                &path,
                "projections",
                &mut view.projections,
            )?;
            additions(
                table.get("collections"),
                resource,
                &path,
                "collections",
                &mut view.collections,
            )?;
            if resources
                .len()
                .saturating_add(view.projections.len())
                .saturating_add(view.collections.len())
                > limits.catalog_resources
            {
                return Err(refusal(
                    &path,
                    "",
                    "aggregate backend additions exceed catalog resource ceiling",
                ));
            }
        }
    }
    Ok(view)
}

/// Parse only descriptor-marked bounded graph integers, rejecting all widenings.
fn narrow(
    value: &Value,
    registry: &Registry,
    path: &str,
    view: &mut BackendExtensions,
) -> Result<(), Refusal> {
    let Value::Table(controls) = value else {
        return Err(refusal(path, "graphdb", "expected graph control table"));
    };
    for (name, value) in controls {
        let key = format!("graphdb.{name}");
        let descriptor = registry
            .get(&key)
            .filter(|descriptor| descriptor.class == SettingClass::Bounded)
            .ok_or_else(|| refusal(path, &key, "not a narrowable graph control"))?;
        let Value::Integer(integer) = value else {
            return Err(refusal(path, &key, "expected unsigned graph integer"));
        };
        let parsed = descriptor
            .kind
            .parse_text(&integer.to_string())
            .map_err(|error| refusal(path, &key, error.to_string()))?;
        let (SettingValue::Integer(integer), Some(SettingValue::Integer(ceiling))) =
            (parsed, registry.default_of(&key))
        else {
            return Err(refusal(path, &key, "not a numeric ceiling"));
        };
        if integer > *ceiling {
            return Err(refusal(path, &key, "cannot raise the base ceiling"));
        }
        view.ceilings
            .entry(key)
            .and_modify(|current| *current = (*current).min(integer))
            .or_insert(integer);
    }
    Ok(())
}

/// Names belong to this package; repeated declarations refuse, even equal ones.
fn additions(
    value: Option<&Value>,
    owner: &Resource,
    path: &str,
    key: &str,
    names: &mut BTreeSet<String>,
) -> Result<(), Refusal> {
    let Some(value) = value else {
        return Ok(());
    };
    let additions = value
        .decode::<Vec<String>>()
        .map_err(|_| refusal(path, key, "expected names"))?;
    for name in additions {
        if name
            .split_once('/')
            .is_none_or(|(namespace, local)| namespace != owner.id.name || !is_name(local))
        {
            return Err(refusal(
                path,
                key,
                "addition must use its own package namespace",
            ));
        }
        if !names.insert(name) {
            return Err(refusal(path, key, "duplicate backend addition"));
        }
    }
    Ok(())
}

/// Activation reuses the sole S1 default producer and checked extension algorithm.
pub(super) fn selected(
    resources: &[&Resource],
    common: Option<&str>,
) -> Result<BackendExtensions, Refusal> {
    let registry = Registry::built_in().map_err(|error| refusal("", "", error.to_string()))?;
    let bases = resources
        .iter()
        .filter(|resource| resource.id.kind == "backend")
        .map(|resource| (*resource).clone())
        .collect::<Vec<_>>();
    let registry = from_resources(&registry, &bases, common, &Limits::PRODUCTION)?;
    effective(resources, &registry, &Limits::PRODUCTION)
}

/// Locate errors without ever echoing literal values from source.
fn refusal(path: &str, key: &str, message: impl Into<String>) -> Refusal {
    Refusal {
        diagnostics: vec![Diagnostic::new(path, key, message)],
    }
}
