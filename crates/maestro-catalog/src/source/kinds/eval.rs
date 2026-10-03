//! Complete inert Phase 1 eval declarations; driver execution belongs to C67.

use super::{contract, standard::nonempty};
use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    parse::is_name,
    placements::safe,
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource, ResourceId, Value},
};
use std::path::Path;

/// Exact declaration shape, without execution or schema-instance evaluation.
pub(super) fn descriptor() -> KindDescriptor {
    let mut fields = contract::fields();
    fields.push(Field::required("driver", FieldType::Text));
    for key in ["subjects", "inputs", "cases", "checks"] {
        fields.push(Field::required(key, FieldType::TextList));
    }
    fields.extend([
        Field::required(
            "expected",
            FieldType::Table {
                fields: vec![
                    Field::required("outputs", FieldType::TextSequence),
                    Field::required("statuses", FieldType::TextSequence),
                ],
            },
        ),
        Field::required(
            "limits",
            FieldType::Table {
                fields: vec![Field::required("timeout_ms", FieldType::Integer)],
            },
        ),
    ]);
    KindDescriptor {
        kind: "eval-case".to_owned(),
        version: 1,
        directory: "evals".to_owned(),
        scopes: vec![Scope::Common, Scope::Core, Scope::Team, Scope::Language],
        layout: Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: None,
        fields,
        body: false,
        requires: vec!["*".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("eval-contracts".to_owned()),
    }
}

/// Static data only, captured once by the bounded source snapshot.
#[derive(Debug)]
pub(super) struct EvalRules;

impl KindRules for EvalRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _: Option<&str>,
        _: Known<'_>,
        problems: &mut Problems,
    ) {
        contract::references(resource, problems);
        if !resource
            .fields
            .get("driver")
            .and_then(Value::text)
            .is_some_and(is_name)
        {
            problems.push((
                "driver".to_owned(),
                "needs a lower-case hyphenated driver name".to_owned(),
            ));
        }
        for key in ["subjects", "checks"] {
            references(resource, key, problems);
        }
        for key in ["inputs", "cases"] {
            nonempty(resource, key, problems);
        }
        expectations(resource, problems);
        if !matches!(
            nested(resource, "limits", "timeout_ms"),
            Some(Value::Integer(1..=1_800_000))
        ) {
            problems.push((
                "limits.timeout_ms".to_owned(),
                "must be an integer in 1..=1800000 (Pi's 30-minute run timeout)".to_owned(),
            ));
        }
    }

    fn assets(&self, resource: &Resource) -> Result<Vec<String>, String> {
        let mut assets = Vec::new();
        for (key, value) in [
            ("inputs", resource.fields.get("inputs")),
            ("cases", resource.fields.get("cases")),
            ("expected.outputs", nested(resource, "expected", "outputs")),
        ] {
            for path in value.and_then(Value::texts).unwrap_or_default() {
                asset_path(key, path)?;
                let parent = resource
                    .path
                    .rsplit_once('/')
                    .map_or("", |(parent, _)| parent);
                assets.push(format!("{parent}/{path}"));
            }
        }
        Ok(assets)
    }
}

/// One already descriptor-checked nested field.
fn nested<'a>(resource: &'a Resource, table: &str, key: &str) -> Option<&'a Value> {
    match resource.fields.get(table) {
        Some(Value::Table(fields)) => fields.get(key),
        _ => None,
    }
}

/// Typed declaration membership feeds the shared qualified reference resolver.
fn references(resource: &Resource, key: &str, problems: &mut Problems) {
    nonempty(resource, key, problems);
    for text in resource
        .fields
        .get(key)
        .and_then(Value::texts)
        .unwrap_or_default()
    {
        let Some(id) =
            ResourceId::parse(text).filter(|id| key != "checks" || id.kind == "standard-check")
        else {
            problems.push((
                key.to_owned(),
                format!(
                    "{text:?} needs a qualified {} ID",
                    if key == "checks" {
                        "standard-check"
                    } else {
                        "resource"
                    }
                ),
            ));
            continue;
        };
        contract::declared(resource, key, &id, problems);
    }
}

/// Ordered outputs and statuses each bind exactly one declared case.
fn expectations(resource: &Resource, problems: &mut Problems) {
    let cases = resource
        .fields
        .get("cases")
        .and_then(Value::texts)
        .unwrap_or_default();
    for key in ["outputs", "statuses"] {
        let values = nested(resource, "expected", key)
            .and_then(Value::texts)
            .unwrap_or_default();
        if values.is_empty() || values.len() != cases.len() {
            problems.push((
                format!("expected.{key}"),
                "must be nonempty, one per case in cases order".to_owned(),
            ));
        }
        if key == "statuses"
            && values
                .iter()
                .any(|value| !["passed", "failed"].contains(value))
        {
            problems.push((
                "expected.statuses".to_owned(),
                "only passed/failed statuses are admitted".to_owned(),
            ));
        }
    }
}

/// Only exact owner-relative JSON paths enter the snapshot inventory.
fn asset_path(key: &str, path: &str) -> Result<(), String> {
    if !safe(path, false) {
        return Err(format!(
            "{key} needs an exact owner-relative asset path: {path:?}"
        ));
    }
    if !Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err(format!("{key} must name an exact JSON asset"));
    }
    Ok(())
}
