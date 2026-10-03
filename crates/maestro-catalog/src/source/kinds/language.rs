//! Root languages extend the shared area contract, without a package alias.

use super::package::{self, PackageRules};
use crate::source::{
    descriptor::{Field, FieldType, KindDescriptor, Scope},
    rules::KindRules,
    types::{Known, Problems, Resource, ResourceId, Value},
};
use std::collections::BTreeMap;

/// Required owner-local profile, instruction and inert starter declarations.
pub(super) fn descriptor() -> KindDescriptor {
    let mut descriptor = package::descriptor("language", vec![Scope::Language]);
    descriptor.version = 5;
    descriptor.hook = Some("language-area".to_owned());
    for field in &mut descriptor.fields {
        if let FieldType::Delegated { validator } = &mut field.kind {
            "language-area".clone_into(validator);
        }
    }
    descriptor.fields.extend([
        Field::required("technology", FieldType::Text),
        Field::required("quality_profile", FieldType::Text),
        Field::required("instructions", FieldType::TextList),
        Field::required("starter", FieldType::TextList),
    ]);
    descriptor
}

/// Shared package validation plus the language's explicit typed reference edges.
#[derive(Debug)]
pub(super) struct LanguageRules;

impl KindRules for LanguageRules {
    fn check_resource(
        &self,
        resource: &Resource,
        body: Option<&str>,
        known: Known<'_>,
        problems: &mut Problems,
    ) {
        PackageRules.check_resource(resource, body, known, problems);
        for (field, kind, references) in references(resource) {
            check_local(resource, field, kind, &references, problems);
        }
    }

    fn check_catalog(
        &self,
        resource: &Resource,
        catalog: &BTreeMap<ResourceId, &Resource>,
        problems: &mut Problems,
    ) {
        PackageRules.check_catalog(resource, catalog, problems);
        for (field, reference) in references(resource)
            .into_iter()
            .flat_map(|(field, _, refs)| refs.into_iter().map(move |reference| (field, reference)))
        {
            if ResourceId::parse(reference).is_some_and(|id| !catalog.contains_key(&id)) {
                problems.push((
                    field.to_owned(),
                    format!("requires {reference}, which does not exist"),
                ));
            }
        }
    }

    fn edges(&self, resource: &Resource) -> Vec<ResourceId> {
        references(resource)
            .into_iter()
            .flat_map(|(_, _, refs)| refs)
            .filter_map(ResourceId::parse)
            .collect()
    }
}

/// The fields themselves declare dependencies; shared closure validation uses these edges.
fn references(resource: &Resource) -> [(&str, &str, Vec<&str>); 3] {
    [
        (
            "quality_profile",
            "quality-profile",
            resource
                .fields
                .get("quality_profile")
                .and_then(Value::text)
                .into_iter()
                .collect(),
        ),
        (
            "instructions",
            "instructions",
            resource
                .fields
                .get("instructions")
                .and_then(Value::texts)
                .unwrap_or_default(),
        ),
        (
            "starter",
            "bootstrap-inventory",
            resource
                .fields
                .get("starter")
                .and_then(Value::texts)
                .unwrap_or_default(),
        ),
    ]
}

/// References cannot change kind, leave the area or omit a required declaration.
fn check_local(
    resource: &Resource,
    field: &str,
    kind: &str,
    references: &[&str],
    problems: &mut Problems,
) {
    if references.is_empty() {
        problems.push((field.to_owned(), "requires a nonempty list".to_owned()));
    }
    for reference in references {
        if ResourceId::parse(reference).is_none_or(|id| {
            id.kind != kind || id.namespace.as_deref() != Some(resource.id.name.as_str())
        }) {
            problems.push((
                field.to_owned(),
                format!("must reference an owner-local {kind}"),
            ));
        }
    }
}
