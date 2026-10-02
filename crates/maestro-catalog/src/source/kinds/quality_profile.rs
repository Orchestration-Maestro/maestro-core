//! Inert quality declarations, separate from session preferences and model profiles.

use crate::{
    settings::{
        Layer,
        resolve::{additive, numeric_narrows},
    },
    source::{
        descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
        rules::KindRules,
        types::{Known, Maturity, Problems, Resource, ResourceId, Value},
    },
};
use maestro_settings::Value as SettingValue;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Quality resources live in their own identity and placement, not profiles/models.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "quality-profile".to_owned(),
        version: 1,
        directory: "profiles/quality".to_owned(),
        scopes: vec![
            Scope::Common,
            Scope::Core,
            Scope::Team,
            Scope::Language,
            Scope::Standard,
        ],
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
            Field::required("subject", FieldType::Text),
            Field::optional("baseline", FieldType::Text),
            Field::required("gates", FieldType::TextList),
            Field::required("applicability", FieldType::TextList),
            Field::required("failure_conditions", FieldType::TextList),
            Field::required("evidence", FieldType::TextList),
            Field::required(
                "bindings",
                FieldType::Delegated {
                    validator: "quality-profile".to_owned(),
                },
            ),
            Field::required(
                "thresholds",
                FieldType::Delegated {
                    validator: "quality-profile".to_owned(),
                },
            ),
        ],
        body: false,
        requires: vec!["*".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("quality-profile".to_owned()),
    }
}

/// A declaration may name an existing resource; it cannot claim runtime qualification.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    /// Missing implementations remain explicitly unresolved.
    state: BindingState,
    /// Exact catalog resource reference, only for a bound declaration.
    reference: Option<String>,
}

/// Binding availability is data, never a successful gate result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BindingState {
    /// No implementation reference has been supplied.
    Unresolved,
    /// A declared reference exists; runtime execution remains outside this checker.
    Bound,
}

/// Numeric restriction direction is explicit data, not inferred from a metric name.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Thresholds {
    /// Minimum scores, such as coverage.
    floors: BTreeMap<String, Threshold>,
    /// Maximum counts, such as warnings.
    ceilings: BTreeMap<String, Threshold>,
}

/// Preserve integer precision and refuse type changes instead of coercing bounds.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Threshold {
    /// Exact whole-number count.
    Integer(i64),
    /// Finite fractional score.
    Float(f64),
}

impl Threshold {
    /// Stable type labels for repair diagnostics.
    fn kind(&self) -> &'static str {
        match self {
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
        }
    }

    /// Validate the trust boundary without losing integer precision.
    fn valid(&self) -> bool {
        match self {
            Self::Integer(value) => *value >= 0,
            Self::Float(value) => *value >= 0.0,
        }
    }

    /// C17 owns the comparison; changing the authored numeric type refuses.
    fn narrows(&self, value: &Self, floor: bool) -> Result<bool, String> {
        match (self, value) {
            (Self::Integer(bound), Self::Integer(value)) => {
                Ok(numeric_narrows(bound, value, floor))
            }
            (Self::Float(bound), Self::Float(value)) => Ok(numeric_narrows(bound, value, floor)),
            _ => Err(format!(
                "changes type from {} to {}",
                self.kind(),
                value.kind()
            )),
        }
    }
}

/// The isolated consumer uses C17 accumulation and its shared numeric direction rule.
#[derive(Debug)]
pub(super) struct QualityRules;

impl KindRules for QualityRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        _known: Known<'_>,
        problems: &mut Problems,
    ) {
        for key in ["gates", "applicability", "failure_conditions", "evidence"] {
            super::standard::nonempty(resource, key, problems);
        }
        match bindings(resource) {
            Ok(bindings) => check_bindings(resource, &bindings, problems),
            Err(message) => problems.push(("bindings".to_owned(), message)),
        }
        match thresholds(resource) {
            Ok(thresholds) => check_thresholds(&thresholds, problems),
            Err(message) => problems.push(("thresholds".to_owned(), message)),
        }
        if let Some(reference) = resource.fields.get("baseline").and_then(Value::text) {
            check_reference(resource, reference, true, problems);
        }
    }

    fn check_catalog(
        &self,
        resource: &Resource,
        catalog: &BTreeMap<ResourceId, &Resource>,
        problems: &mut Problems,
    ) {
        let Some(baseline) = resource
            .fields
            .get("baseline")
            .and_then(Value::text)
            .and_then(ResourceId::parse)
            .and_then(|id| catalog.get(&id))
        else {
            return;
        };
        if baseline.fields.contains_key("baseline") {
            problems.push((
                "baseline".to_owned(),
                "baseline depth exceeds one".to_owned(),
            ));
            return;
        }
        retain_gates(resource, baseline, problems);
        if let (Ok(child), Ok(parent)) = (thresholds(resource), thresholds(baseline)) {
            narrow(&parent.floors, &child.floors, true, problems);
            narrow(&parent.ceilings, &child.ceilings, false, problems);
        }
    }
}

/// Decode the hook-owned exact nested binding shape.
fn bindings(resource: &Resource) -> Result<BTreeMap<String, Binding>, String> {
    let value = resource
        .fields
        .get("bindings")
        .ok_or_else(|| "missing bindings".to_owned())?;
    let Value::Table(records) = value else {
        return Err("bindings must be a table".to_owned());
    };
    for (gate, record) in records {
        if !matches!(record, Value::Table(_)) {
            return Err(format!("binding {gate} must be a table"));
        }
    }
    value.decode()
}

/// Decode both explicit restriction directions without supplying defaults.
fn thresholds(resource: &Resource) -> Result<Thresholds, String> {
    let value = resource
        .fields
        .get("thresholds")
        .ok_or_else(|| "missing thresholds".to_owned())?;
    if !matches!(value, Value::Table(_)) {
        return Err("thresholds must be a table".to_owned());
    }
    value.decode()
}

/// Every required gate records availability, even if its implementation is absent.
fn check_bindings(
    resource: &Resource,
    bindings: &BTreeMap<String, Binding>,
    problems: &mut Problems,
) {
    let gates = resource
        .fields
        .get("gates")
        .and_then(Value::texts)
        .unwrap_or_default();
    for gate in &gates {
        if !bindings.contains_key(*gate) {
            problems.push((
                "bindings".to_owned(),
                format!("missing explicit binding for {gate}"),
            ));
        }
    }
    for (gate, binding) in bindings {
        if !gates.contains(&gate.as_str()) {
            problems.push((
                "bindings".to_owned(),
                format!("binding for undeclared gate {gate}"),
            ));
        }
        match (&binding.state, &binding.reference) {
            (BindingState::Unresolved, Some(_)) => problems.push((
                "bindings".to_owned(),
                "unresolved binding cannot claim a reference".to_owned(),
            )),
            (BindingState::Bound, None) => problems.push((
                "bindings".to_owned(),
                "bound binding requires an exact reference".to_owned(),
            )),
            (BindingState::Bound, Some(reference)) => {
                check_reference(resource, reference, false, problems);
            }
            (BindingState::Unresolved, None) => {}
        }
    }
}

/// References cannot bypass shared layer, existence or closure validation.
fn check_reference(resource: &Resource, text: &str, baseline: bool, problems: &mut Problems) {
    let id = ResourceId::parse(text);
    if baseline && id.as_ref().is_none_or(|id| id.kind != "quality-profile") {
        problems.push((
            "baseline".to_owned(),
            "baseline must reference a quality-profile".to_owned(),
        ));
    } else if id.is_none() {
        problems.push((
            "bindings".to_owned(),
            "binding must be an exact typed reference".to_owned(),
        ));
    }
    if id.is_some_and(|id| !resource.metadata.requires.contains(&id)) {
        problems.push((
            "requires".to_owned(),
            format!("{text} must be declared in metadata.requires"),
        ));
    }
}

/// Nonnegative finite bounds must have a nonempty metric and a nonempty intersection.
fn check_thresholds(thresholds: &Thresholds, problems: &mut Problems) {
    for (metric, value) in thresholds.floors.iter().chain(&thresholds.ceilings) {
        if metric.trim().is_empty() || !value.valid() {
            problems.push((
                "thresholds".to_owned(),
                "thresholds require nonempty metrics and nonnegative finite values".to_owned(),
            ));
        }
    }
    for (metric, floor) in &thresholds.floors {
        if thresholds
            .ceilings
            .get(metric)
            .is_some_and(|ceiling| floor.narrows(ceiling, true) != Ok(true))
        {
            problems.push((
                "thresholds".to_owned(),
                format!("conflicting threshold {metric}"),
            ));
        }
    }
}

/// Accumulate through C17; refusal, rather than silent restoration, protects required gates.
fn retain_gates(resource: &Resource, baseline: &Resource, problems: &mut Problems) {
    let parent = baseline
        .fields
        .get("gates")
        .and_then(Value::texts)
        .unwrap_or_default();
    let child = resource
        .fields
        .get("gates")
        .and_then(Value::texts)
        .unwrap_or_default();
    let list =
        |items: &[&str]| SettingValue::List(items.iter().map(|item| (*item).to_owned()).collect());
    let accumulated = match additive(
        "gates",
        list(&parent),
        vec![(Layer::Standard, list(&child))],
    ) {
        Ok(accumulated) => accumulated,
        Err(problem) => {
            problems.push((problem.key, problem.message));
            return;
        }
    };
    if let SettingValue::List(gates) = accumulated.value() {
        for gate in gates.iter().filter(|gate| !child.contains(&gate.as_str())) {
            problems.push(("gates".to_owned(), format!("removed required gate {gate}")));
        }
    }
}

/// Every inherited bound remains present and moves only in its restrictive direction.
fn narrow(
    parent: &BTreeMap<String, Threshold>,
    child: &BTreeMap<String, Threshold>,
    floor: bool,
    problems: &mut Problems,
) {
    let direction = if floor { "floor" } else { "ceiling" };
    for (metric, bound) in parent {
        match child.get(metric) {
            None => problems.push((
                "thresholds".to_owned(),
                format!("removed {direction} {metric}"),
            )),
            Some(value) => match bound.narrows(value, floor) {
                Ok(true) => {}
                Ok(false) => problems.push((
                    "thresholds".to_owned(),
                    format!("weakens {direction} {metric}"),
                )),
                Err(message) => problems.push((
                    "thresholds".to_owned(),
                    format!("threshold {metric} {message}"),
                )),
            },
        }
    }
}
