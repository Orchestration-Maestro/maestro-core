//! Inert standard-local machine checks: identities, registered adapters and inputs.

use super::standard::{duplicate_rules, nonempty};
use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource, ResourceId, Value},
};
use std::collections::BTreeMap;

/// A declarative machine check, never an executable or source-provided validator.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "standard-check".to_owned(),
        version: 1,
        directory: "checks".to_owned(),
        scopes: vec![Scope::Standard],
        layout: Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: Some("name".to_owned()),
        fields: vec![
            Field::required("name", FieldType::Text),
            Field::required("rules", FieldType::TextList),
            Field::required("validator", FieldType::Text),
            Field::required("applicability", FieldType::TextList),
            Field::required("inputs", FieldType::TextList),
            Field::required("evidence", FieldType::TextList),
            Field::required("refusals", FieldType::TextList),
        ],
        body: false,
        requires: vec!["*".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("standard-check".to_owned()),
    }
}

/// Check declarations bind the existing fixed hook table, without invoking it.
#[derive(Debug)]
pub(super) struct StandardCheckRules {
    /// Membership in the existing fixed hook table, supplied at registration.
    pub(super) known_validator: fn(&str) -> bool,
}

impl KindRules for StandardCheckRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        _known: Known<'_>,
        problems: &mut Problems,
    ) {
        for key in ["rules", "applicability", "inputs", "evidence", "refusals"] {
            nonempty(resource, key, problems);
        }
        if !resource
            .fields
            .get("validator")
            .and_then(Value::text)
            .is_some_and(self.known_validator)
        {
            problems.push(("validator".to_owned(), "unregistered validator".to_owned()));
        }
        for input in resource
            .fields
            .get("inputs")
            .and_then(Value::texts)
            .unwrap_or_default()
        {
            if ResourceId::parse(input).is_none() {
                problems.push((
                    "inputs".to_owned(),
                    format!("{input} is not a typed qualified ID"),
                ));
            }
        }
    }

    fn check_catalog(
        &self,
        resource: &Resource,
        catalog: &BTreeMap<ResourceId, &Resource>,
        problems: &mut Problems,
    ) {
        duplicate_rules(resource, catalog, problems);
        let standard = ResourceId {
            kind: "standard".to_owned(),
            namespace: None,
            name: resource.id.namespace.clone().unwrap_or_default(),
        };
        let inventory = catalog
            .get(&standard)
            .and_then(|standard| standard.fields.get("rules"))
            .and_then(Value::texts)
            .unwrap_or_default();
        for rule in resource
            .fields
            .get("rules")
            .and_then(Value::texts)
            .unwrap_or_default()
        {
            if !inventory.contains(&rule) {
                problems.push((
                    "rules".to_owned(),
                    format!("{rule} is not in {standard} inventory"),
                ));
            }
        }
        // Shared reference/layer validation and closure traversal own these edges.
        for input in resource
            .fields
            .get("inputs")
            .and_then(Value::texts)
            .unwrap_or_default()
            .into_iter()
            .filter_map(ResourceId::parse)
        {
            if !resource.metadata.requires.contains(&input) {
                problems.push((
                    "inputs".to_owned(),
                    format!("{input} must be declared in metadata.requires"),
                ));
            }
        }
    }
}
