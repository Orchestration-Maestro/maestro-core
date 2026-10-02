//! Mandatory standard area roots and their normative rule inventories.
//! References use the shared typed `metadata.requires` envelope.

use super::package::{self, PackageRules};
use crate::source::standards;
use crate::source::{
    descriptor::{Field, FieldType, KindDescriptor, Scope},
    rules::KindRules,
    types::{Known, Problems, Resource, ResourceId, Value},
};
use std::collections::BTreeMap;

/// A standard is mandatory by kind; no optional/disable field is admitted.
pub(super) fn descriptor() -> KindDescriptor {
    let mut descriptor = package::descriptor("standard", vec![Scope::Standard]);
    descriptor.version = 4;
    descriptor
        .fields
        .retain(|field| field.key != "settings" && field.key != "exceptions");
    descriptor
        .fields
        .extend(standards::fields("standard-inventory"));
    descriptor
        .fields
        .push(Field::optional("non_negotiable", FieldType::TextList));
    descriptor
        .fields
        .push(Field::required("rules", FieldType::TextList));
    descriptor.hook = Some("standard-inventory".to_owned());
    descriptor
}

/// Shape and identity checks only: normative rules remain catalog data.
#[derive(Debug)]
pub(super) struct StandardRules;

impl KindRules for StandardRules {
    fn check_resource(
        &self,
        resource: &Resource,
        body: Option<&str>,
        known: Known<'_>,
        problems: &mut Problems,
    ) {
        PackageRules.check_resource(resource, body, known, problems);
        nonempty(resource, "rules", problems);
        let rules = resource
            .fields
            .get("rules")
            .and_then(Value::texts)
            .unwrap_or_default();
        for rule in resource
            .fields
            .get("non_negotiable")
            .and_then(Value::texts)
            .unwrap_or_default()
        {
            if !rules.contains(&rule) {
                problems.push((
                    "non_negotiable".to_owned(),
                    format!("{rule} not in rule inventory"),
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
    }
}

/// A present list must have content; the strict descriptor checks its item shape.
pub(super) fn nonempty(resource: &Resource, key: &str, problems: &mut Problems) {
    if resource
        .fields
        .get(key)
        .and_then(Value::texts)
        .is_none_or(|items| items.is_empty())
    {
        problems.push((key.to_owned(), "must be a nonempty list".to_owned()));
    }
}

/// Standards claim inventory IDs; checks reference them but claim a single check slot.
pub(super) fn duplicate_rules(
    resource: &Resource,
    catalog: &BTreeMap<ResourceId, &Resource>,
    problems: &mut Problems,
) {
    let rules = resource
        .fields
        .get("rules")
        .and_then(Value::texts)
        .unwrap_or_default();
    // ponytail: bounded pairwise scan; index rule IDs if large catalogs need it.
    for other in catalog
        .values()
        .filter(|other| other.id.kind == resource.id.kind && other.id < resource.id)
    {
        let previous = other
            .fields
            .get("rules")
            .and_then(Value::texts)
            .unwrap_or_default();
        for rule in rules.iter().filter(|rule| previous.contains(rule)) {
            problems.push((
                "rules".to_owned(),
                format!("duplicate rule identity {rule}, also {}", other.id),
            ));
        }
    }
}
