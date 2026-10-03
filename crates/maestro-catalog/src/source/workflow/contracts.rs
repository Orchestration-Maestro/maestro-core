//! Real JSON Schema validation over the loader's single parsed snapshot.

use super::super::{
    load::{Loaded, Native},
    types::{Diagnostic, Resource, ResourceId},
};
use jsonschema::{Draft, Registry, Validator};
use serde_json::Value;
use std::{collections::BTreeMap, ptr};

/// Check each admitted contract against the same in-memory resource registry.
pub(crate) fn check(loaded: &[Loaded]) -> Vec<Diagnostic> {
    let mut schemas = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for loaded in loaded
        .iter()
        .filter(|loaded| loaded.resource.id.kind == "contract")
    {
        match &loaded.native {
            Native::Json(schema) => {
                schemas.insert(loaded.resource.id.clone(), (&loaded.resource, schema));
            }
            Native::None => diagnostics.push(Diagnostic::new(
                &loaded.resource.path,
                "contract",
                "contract needs native JSON input",
            )),
            Native::Cedar(text) => diagnostics.push(Diagnostic::new(
                &loaded.resource.path,
                "contract",
                format!("contract needs JSON, not Cedar ({} bytes)", text.len()),
            )),
        }
    }
    for (resource, schema) in schemas.values() {
        if let Err(message) = references(resource, schema, &schemas) {
            diagnostics.push(Diagnostic::new(&resource.path, "contract", message));
        }
    }
    if !diagnostics.is_empty() || schemas.is_empty() {
        return diagnostics;
    }
    let registry = match registry(&schemas) {
        Ok(registry) => registry,
        Err(message) => {
            return schemas
                .values()
                .map(|(resource, _)| Diagnostic::new(&resource.path, "contract", &message))
                .collect();
        }
    };
    for (resource, schema) in schemas.values() {
        if let Err(message) = validator(schema, &registry) {
            diagnostics.push(Diagnostic::new(&resource.path, "contract", message));
        }
    }
    diagnostics
}

/// Schema positions come from the real draft walker, not an instance-data scan.
fn references(
    resource: &Resource,
    schema: &Value,
    schemas: &BTreeMap<ResourceId, (&Resource, &Value)>,
) -> Result<(), String> {
    header(resource, schema)?;
    let root = schema;
    let mut pending = vec![schema];
    while let Some(schema) = pending.pop() {
        if !ptr::eq(schema, root) && schema.get("$id").is_some() {
            let pointer = pointer(root, schema).unwrap_or_default();
            return Err(format!(
                "{pointer}/$id is unsupported: only the primary contract identity is admitted; \
                 use $defs with $anchor or fragment references"
            ));
        }
        for key in ["$ref", "$dynamicRef"] {
            if let Some(text) = schema.get(key).and_then(Value::as_str) {
                reference(resource, text, schemas)?;
            }
        }
        pending.extend(Draft::Draft202012.subresources_of(schema));
    }
    Ok(())
}

/// The primary schema has one exact resource identity and one supported dialect.
fn header(resource: &Resource, schema: &Value) -> Result<(), String> {
    if schema.get("$schema").and_then(Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
        || schema.get("type").and_then(Value::as_str) != Some("object")
    {
        return Err("contract must declare JSON Schema 2020-12 and type object".to_owned());
    }
    if schema.get("$id").and_then(Value::as_str) != Some(resource.id.to_string().as_str()) {
        return Err(format!("$id must equal {}", resource.id));
    }
    Ok(())
}

/// Refuse aliases and require every different contract in the declared snapshot closure.
fn reference(
    resource: &Resource,
    text: &str,
    schemas: &BTreeMap<ResourceId, (&Resource, &Value)>,
) -> Result<(), String> {
    let target = text.split('#').next().unwrap_or_default();
    if target.is_empty() && text.starts_with('#') {
        return Ok(());
    }
    let id = ResourceId::parse(target)
        .filter(|id| id.kind == "contract")
        .ok_or_else(|| {
            format!(
                "reference {text:?} needs a fragment or qualified contract ID; \
            paths and external retrieval refuse"
            )
        })?;
    if id != resource.id && !resource.metadata.requires.contains(&id) {
        return Err(format!(
            "reference {text:?}: {id} must be declared in requires"
        ));
    }
    if !schemas.contains_key(&id) {
        return Err(format!(
            "reference {text:?}: {id} is missing from the checked snapshot"
        ));
    }
    Ok(())
}

/// Locate an already identified schema node only when its refusal needs a pointer.
fn pointer(root: &Value, target: &Value) -> Option<String> {
    let mut pending = vec![(root, String::new())];
    while let Some((value, pointer)) = pending.pop() {
        if ptr::eq(value, target) {
            return Some(pointer);
        }
        match value {
            Value::Object(fields) => pending.extend(fields.iter().map(|(key, value)| {
                let key = key.replace('~', "~0").replace('/', "~1");
                (value, format!("{pointer}/{key}"))
            })),
            Value::Array(items) => pending.extend(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, value)| (value, format!("{pointer}/{index}"))),
            ),
            _ => {}
        }
    }
    None
}

/// All documents are already parsed and exact-ID checked before registration.
fn registry<'a>(
    schemas: &BTreeMap<ResourceId, (&Resource, &'a Value)>,
) -> Result<Registry<'a>, String> {
    let mut builder = Registry::new().draft(Draft::Draft202012);
    for (id, (_, schema)) in schemas {
        builder = builder
            .add(id.to_string(), *schema)
            .map_err(|error| error.to_string())?;
    }
    builder.prepare().map_err(|error| error.to_string())
}

/// Build one reusable real validator; no network or filesystem retrieval is admitted.
pub(super) fn validator(schema: &Value, registry: &Registry<'_>) -> Result<Validator, String> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(registry)
        .offline()
        .build(schema)
        .map_err(|error| error.to_string())
}
