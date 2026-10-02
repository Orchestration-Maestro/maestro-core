//! One declarative backend kind with strict role tables checked by its hook.

use crate::source::{
    backends::BACKENDS,
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource, Value},
};
use maestro_settings::Registry;
use std::{collections::BTreeMap, iter::once};

/// Exact fixed core role paths; products are type values, never resource IDs.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "backend".to_owned(),
        version: 1,
        scopes: vec![Scope::Core],
        directory: "backends".to_owned(),
        layout: Layout::Folder {
            file: "config.toml".to_owned(),
            data: vec![],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: None,
        fields: once(Field::required("type", FieldType::Text))
            .chain(BACKENDS.iter().map(|backend| {
                Field::optional(
                    backend.role,
                    FieldType::Delegated {
                        validator: "backend-base".to_owned(),
                    },
                )
            }))
            .collect(),
        body: false,
        requires: vec![],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("backend-base".to_owned()),
    }
}

/// Build-independent strict decoding; availability belongs to runtime selection.
#[derive(Debug)]
pub(super) struct BackendRules;

impl KindRules for BackendRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        _known: Known<'_>,
        problems: &mut Problems,
    ) {
        let Some(backend) = BACKENDS
            .iter()
            .find(|backend| backend.role == resource.id.name)
        else {
            problems.push(("role".to_owned(), "unknown backend role".to_owned()));
            return;
        };
        if !resource
            .fields
            .get("type")
            .and_then(Value::text)
            .is_some_and(|kind| backend.types.contains(&kind))
        {
            problems.push((
                "type".to_owned(),
                "unknown backend type for this role".to_owned(),
            ));
        }
        let registry = match Registry::built_in() {
            Ok(registry) => registry,
            Err(error) => {
                problems.push(("graphdb".to_owned(), error.to_string()));
                return;
            }
        };
        for registered in BACKENDS {
            let Some(value) = resource.fields.get(registered.role) else {
                continue;
            };
            if registered.role != backend.role {
                problems.push((
                    registered.role.to_owned(),
                    "table belongs to another backend role".to_owned(),
                ));
            }
            check_controls(value, registered.role, &registry, problems);
        }
    }
}

/// Decode every table as unsigned numeric controls and reject every undeclared key.
/// Even a disabled graph's table passes through this exact decoder and bound data.
fn check_controls(value: &Value, role: &str, registry: &Registry, problems: &mut Problems) {
    let known = |key: &str| role == "graphdb" && registry.get(&format!("{role}.{key}")).is_some();
    let Ok(controls) = value.decode::<BTreeMap<String, u64>>() else {
        // Preserve unknown-key diagnostics without exposing literal values.
        if let Value::Table(table) = value {
            for key in table.keys().filter(|key| !known(key)) {
                problems.push((format!("{role}.{key}"), "unknown key".to_owned()));
            }
        }
        problems.push((
            role.to_owned(),
            "must be a table of unsigned integers".to_owned(),
        ));
        return;
    };
    for (key, value) in controls {
        let key = format!("{role}.{key}");
        let Some(descriptor) = registry.get(&key).filter(|_| role == "graphdb") else {
            problems.push((key, "unknown key".to_owned()));
            continue;
        };
        if let Err(error) = descriptor.kind.parse_text(&value.to_string()) {
            problems.push((key, error.to_string()));
        }
    }
}
