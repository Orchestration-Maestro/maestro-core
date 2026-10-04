//! Native JSON Schema contracts with an exact owner-local metadata pair.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    types::{Maturity, Problems, Resource, ResourceId, Value},
};

/// Contract placement and metadata; the admitted JSON consumer owns semantics.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "contract".to_owned(),
        version: 1,
        directory: "contracts".to_owned(),
        scopes: vec![Scope::Common, Scope::Core, Scope::Team, Scope::Language],
        layout: Layout::Files {
            suffix: ".schema.json".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Json,
        metadata: MetadataPlace::Sidecar {
            suffix: ".maestro.toml".to_owned(),
        },
        name_field: None,
        fields: vec![],
        body: false,
        requires: vec!["contract".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: None,
    }
}

/// The two exact contract references shared by prompts, handoffs and eval cases.
pub(super) fn fields() -> Vec<Field> {
    vec![
        Field::required("description", FieldType::Text),
        Field::required("input_contract", FieldType::Text),
        Field::required("output_contract", FieldType::Text),
    ]
}

/// References must be qualified and declared so the common reference checker
/// owns existence, dependency direction and closure traversal.
pub(super) fn reference(resource: &Resource, key: &str, kind: &str, problems: &mut Problems) {
    let Some(text) = resource.fields.get(key).and_then(Value::text) else {
        return;
    };
    let Some(id) = ResourceId::parse(text).filter(|id| id.kind == kind) else {
        problems.push((
            key.to_owned(),
            format!("{text:?} needs a qualified {kind} ID; paths and external references refuse"),
        ));
        return;
    };
    declared(resource, key, &id, problems);
}

/// Shared declaration membership; the catalog resolver checks the target.
pub(super) fn declared(resource: &Resource, key: &str, id: &ResourceId, problems: &mut Problems) {
    if !resource.metadata.requires.contains(id) {
        problems.push((
            key.to_owned(),
            format!("{id} must be declared in metadata.requires"),
        ));
    }
}

/// Admit both contract fields without reading or parsing a second schema.
pub(super) fn references(resource: &Resource, problems: &mut Problems) {
    for key in ["input_contract", "output_contract"] {
        reference(resource, key, "contract", problems);
    }
}
