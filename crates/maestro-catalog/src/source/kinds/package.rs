//! Area closure roots; the shared checker enforces dependency layers and
//! preset membership. Selection admission checks the mandatory root closure.

use crate::source::{backend_extensions, standards, versions};
use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource, ResourceId, Value},
};
use serde::Deserialize;
use std::collections::BTreeMap;

/// One area kind and its registered roots, never a language/standard package alias.
pub(super) fn descriptor(kind: &str, scopes: Vec<Scope>) -> KindDescriptor {
    let mut descriptor = KindDescriptor {
        kind: kind.to_owned(),
        version: if kind == "package" { 5 } else { 4 },
        directory: String::new(),
        scopes,
        layout: Layout::Area {
            file: "package.toml".to_owned(),
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: Some("name".to_owned()),
        fields: vec![
            Field::required("kind", FieldType::Text),
            Field::required("name", FieldType::Text),
            Field::required("version", FieldType::Text),
            Field::required("owners", FieldType::TextList),
            Field::optional("maintainers", FieldType::TextList),
            Field::required("description", FieldType::Text),
            Field::required("status", FieldType::Text),
            Field::optional("runtime", FieldType::Text),
            Field::optional("dependency_pins", FieldType::ScalarTable),
        ],
        body: false,
        requires: vec!["*".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: true,
        required: None,
        hook: Some("area-package".to_owned()),
    };
    if kind == "package" {
        descriptor.fields.push(Field::optional(
            "backend_extensions",
            FieldType::TextSequence,
        ));
    }
    descriptor.fields.extend(standards::fields("area-package"));
    descriptor
}

/// Lifecycle status is distinct from the evidence maturity in the envelope.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    /// Available for selection.
    Active,
    /// Kept for existing consumers.
    Deprecated,
    /// Withdrawn from new selections.
    Retired,
}

/// Minimal area shape checks; cross-resource layers belong to the shared checker.
#[derive(Debug)]
pub(super) struct PackageRules;

impl KindRules for PackageRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        _known: Known<'_>,
        problems: &mut Problems,
    ) {
        versions::check_resource(resource, problems);
        if let Err(message) = backend_extensions::selectors(resource) {
            problems.push(("backend_extensions".to_owned(), message));
        }
        if resource.fields.get("kind").and_then(Value::text) != Some(resource.id.kind.as_str()) {
            problems.push((
                "kind".to_owned(),
                "must match the area path kind".to_owned(),
            ));
        }
        if resource
            .fields
            .get("version")
            .and_then(Value::text)
            .is_none_or(|version| versions::exact_version(version).is_err())
        {
            problems.push((
                "version".to_owned(),
                "must be an exact SemVer version".to_owned(),
            ));
        }
        if resource.metadata.version.as_deref().is_some_and(|version| {
            Some(version) != resource.fields.get("version").and_then(Value::text)
        }) {
            problems.push((
                "metadata.version".to_owned(),
                "must equal the top-level version".to_owned(),
            ));
        }
        if resource
            .fields
            .get("status")
            .is_none_or(|status| status.decode::<Status>().is_err())
        {
            problems.push((
                "status".to_owned(),
                "must be active, deprecated or retired".to_owned(),
            ));
        }
    }

    fn check_catalog(
        &self,
        resource: &Resource,
        catalog: &BTreeMap<ResourceId, &Resource>,
        problems: &mut Problems,
    ) {
        versions::check_catalog(resource, catalog, problems);
    }

    fn assets(&self, resource: &Resource) -> Result<Vec<String>, String> {
        backend_extensions::assets(resource)
    }

    fn edges(&self, resource: &Resource) -> Vec<ResourceId> {
        backend_extensions::selectors(resource)
            .unwrap_or_default()
            .into_iter()
            .map(|role| ResourceId {
                kind: "backend".to_owned(),
                namespace: Some("core".to_owned()),
                name: role,
            })
            .collect()
    }
}
