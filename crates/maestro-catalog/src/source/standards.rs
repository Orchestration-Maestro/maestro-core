//! Standard settings use the S1 descriptors and the single C17 resolver.

use super::{
    descriptor::{Field, FieldType},
    rules::KindRules,
    types::{Diagnostic, Known, Problems, Resource, ResourceId, Value},
};
use crate::settings::resolve::resolve_with_standards;
use maestro_settings::{Flag, Layers, Registry, Value as SettingValue};
use std::collections::BTreeMap;
use toml::de::DeTable;

/// Strict S1-decoded settings and exact exception references, shared by consumers.
pub(super) fn fields(hook: &str) -> [Field; 2] {
    [
        Field::optional(
            "settings",
            FieldType::Delegated {
                validator: hook.to_owned(),
            },
        ),
        Field::optional("exceptions", FieldType::TextList),
    ]
}

/// Instructions need the delegated settings hook, but no separate resolver.
#[derive(Debug)]
pub(super) struct SettingsRules;
impl KindRules for SettingsRules {}

/// Decode settings against the real registry, refusing names-only adapters and local controls.
fn values(
    resource: &Resource,
    registry: Option<&Registry>,
) -> Result<BTreeMap<String, SettingValue>, String> {
    let Some(value) = resource.fields.get("settings") else {
        return Ok(BTreeMap::new());
    };
    let Some(registry) = registry else {
        return Err("settings require typed descriptors".to_owned());
    };
    let Value::Table(entries) = value else {
        return Err("settings must be a table".to_owned());
    };
    let mut result = BTreeMap::new();
    for (key, value) in entries {
        let descriptor = registry
            .get(key)
            .ok_or_else(|| format!("unknown setting {key}"))?;
        let text = toml::to_string(&BTreeMap::from([("value", value)]))
            .map_err(|error| error.to_string())?;
        let table = DeTable::parse(&text).map_err(|error| error.to_string())?;
        let parsed = table
            .get_ref()
            .get("value")
            .ok_or("missing setting value")?;
        result.insert(
            key.clone(),
            descriptor
                .kind
                .parse_toml(parsed.get_ref())
                .map_err(|error| format!("{key}: {error}"))?,
        );
    }
    Ok(result)
}

/// Validate and combine mandatory standards before checking each consumer's requests.
pub(super) fn check(resources: &[&Resource], known: Known<'_>) -> Vec<Diagnostic> {
    let registry = known.settings.registry();
    let mut diagnostics = Vec::new();
    let mut decoded = Vec::new();
    let mut standards: BTreeMap<String, Vec<SettingValue>> = BTreeMap::new();
    for resource in resources {
        let values = match values(resource, registry) {
            Ok(values) => values,
            Err(message) => {
                diagnostics.push(Diagnostic::new(&resource.path, "settings", message));
                continue;
            }
        };
        if resource.id.kind == "standard" {
            for (key, value) in &values {
                standards
                    .entry(key.clone())
                    .or_default()
                    .push(value.clone());
            }
        }
        decoded.push((*resource, values));
    }
    let Some(registry) = registry else {
        return diagnostics;
    };
    for (resource, values) in decoded {
        let flags: Vec<_> = if resource.id.kind == "standard" {
            Vec::new()
        } else {
            values
                .into_iter()
                .map(|(key, value)| Flag { key, value })
                .collect()
        };
        let layers = maestro_settings::resolve(registry, &Layers::default(), &flags);
        let resolved = resolve_with_standards(registry, &layers, &standards);
        for descriptor in registry.descriptors() {
            if let Some(Err(problem)) = resolved.get(&descriptor.key) {
                diagnostics.push(Diagnostic::new(
                    &resource.path,
                    format!("settings.{}", problem.key),
                    &problem.message,
                ));
            }
        }
    }
    diagnostics
}

/// Effect-free validation, not self-authorized approval.
#[derive(Debug)]
pub(super) struct ExceptionRules;

impl KindRules for ExceptionRules {
    /// Validate a central record with an injected UTC epoch day. Expiry is exclusive.
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        known: Known<'_>,
        problems: &mut Problems,
    ) {
        let parts: Vec<_> = resource.path.split('/').collect();
        let central = matches!(
            parts.as_slice(),
            ["exceptions", _] | ["standards", _, "exceptions", _]
        );
        if !central {
            problems.push((String::new(), "local exception refuses".to_owned()));
        }

        let expiry = resource
            .fields
            .get("expiry")
            .and_then(Value::text)
            .unwrap_or_default();
        if let Err(message) = check_expiry(expiry, known.today) {
            problems.push(("expiry".to_owned(), message.to_owned()));
        }
        let scopes = resource
            .fields
            .get("scopes")
            .and_then(Value::texts)
            .unwrap_or_default();
        if scopes.is_empty()
            || scopes
                .iter()
                .any(|scope| ResourceId::parse(scope).is_none())
        {
            problems.push((
                "scopes".to_owned(),
                "must name nonempty exact qualified IDs".to_owned(),
            ));
        }
        if resource
            .fields
            .get("evidence")
            .and_then(Value::text)
            .and_then(ResourceId::parse)
            .is_none()
        {
            problems.push((
                "evidence".to_owned(),
                "must be a typed evidence reference".to_owned(),
            ));
        }
    }

    /// Rules are inventory data; non-negotiable entries cannot receive exceptions.
    fn check_catalog(
        &self,
        resource: &Resource,
        catalog: &BTreeMap<ResourceId, &Resource>,
        problems: &mut Problems,
    ) {
        let rule = resource
            .fields
            .get("rule")
            .and_then(Value::text)
            .unwrap_or_default();
        let standard = catalog.values().find(|standard| {
            standard.id.kind == "standard"
                && standard
                    .fields
                    .get("rules")
                    .and_then(Value::texts)
                    .unwrap_or_default()
                    .contains(&rule)
        });
        match standard {
            None => problems.push(("rule".to_owned(), "unknown inventory rule".to_owned())),
            Some(standard) => {
                if standard
                    .fields
                    .get("non_negotiable")
                    .and_then(Value::texts)
                    .unwrap_or_default()
                    .contains(&rule)
                {
                    problems.push((
                        "rule".to_owned(),
                        "non-negotiable rule accepts no exception".to_owned(),
                    ));
                }
                if resource.id.namespace.as_deref() != Some("common")
                    && resource.id.namespace != Some(standard.id.name.clone())
                {
                    problems.push((
                        "rule".to_owned(),
                        "exception belongs to a different standard".to_owned(),
                    ));
                }
            }
        }
        for scope in resource
            .fields
            .get("scopes")
            .and_then(Value::texts)
            .unwrap_or_default()
        {
            if ResourceId::parse(scope).is_none_or(|id| !catalog.contains_key(&id)) {
                problems.push(("scopes".to_owned(), format!("unknown scope {scope}")));
            }
        }
    }
}

/// Consumers reference central records, never repeat or widen their contents.
pub(super) fn check_references(
    resource: &Resource,
    catalog: &BTreeMap<ResourceId, &Resource>,
    problems: &mut Problems,
) {
    for reference in resource
        .fields
        .get("exceptions")
        .and_then(Value::texts)
        .unwrap_or_default()
    {
        let exception = ResourceId::parse(reference)
            .filter(|id| id.kind == "standard-exception")
            .and_then(|id| catalog.get(&id));
        match exception {
            None => problems.push((
                "exceptions".to_owned(),
                format!("unknown central exception {reference}"),
            )),
            Some(exception) => {
                if !exception
                    .fields
                    .get("scopes")
                    .and_then(Value::texts)
                    .unwrap_or_default()
                    .contains(&resource.id.to_string().as_str())
                {
                    problems.push((
                        "exceptions".to_owned(),
                        "consumer outside approved scopes".to_owned(),
                    ));
                }
            }
        }
    }
}

/// An exception ceases to apply on its expiry day, not the day after.
fn check_expiry(text: &str, today: i64) -> Result<(), &'static str> {
    match epoch_day(text) {
        Some(expiry) if expiry > today => Ok(()),
        Some(_) => Err("expired central exception"),
        None => Err("must be a YYYY-MM-DD calendar date"),
    }
}

/// Strict Gregorian date to UTC epoch day; no clock or date dependency.
fn epoch_day(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 10
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return None;
    }
    let year: i64 = text.get(..4)?.parse().ok()?;
    let month: usize = text.get(5..7)?.parse().ok()?;
    let day: i64 = text.get(8..)?.parse().ok()?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let last = *months.get(month.checked_sub(1)?)?;
    if year == 0 || day < 1 || day > last {
        return None;
    }
    // Count whole Gregorian years and days preceding this month from 0001-01-01.
    let previous = year - 1;
    Some(
        previous * 365 + previous / 4 - previous / 100
            + previous / 400
            + months.get(..month - 1)?.iter().sum::<i64>()
            + day
            - 1
            - 719_162,
    )
}
