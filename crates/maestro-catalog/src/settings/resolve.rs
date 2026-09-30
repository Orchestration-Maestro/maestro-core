//! Typed, restrictive resolution over four preference layers; storage and parsing stay in S1.

use super::classes::{SettingClass, SettingClasses};
use maestro_settings::{LayerName, Registry, Resolved, ResolvedSetting, Source, Value};
use std::collections::BTreeMap;

/// A chosen value with its source and values it overrode.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedValue {
    /// The effective typed value.
    value: Value,
    /// The layer that supplied or constrained the value.
    source: &'static str,
    /// Lower-priority or ignored values, in precedence order.
    overridden: Vec<(&'static str, Value)>,
}

/// A setting could not be resolved because it was locked or its value widened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveDiagnostic {
    /// The setting key.
    pub key: String,
    /// Why the value was refused or ignored.
    pub message: String,
}

/// All effective values and diagnostics from a resolution pass.
#[derive(Debug, Clone, Default)]
pub struct ResolvedSettings {
    /// Every registry key's resolution result.
    values: BTreeMap<String, Result<ResolvedValue, ResolveDiagnostic>>,
    /// Ignored requests that would widen a restrictive value.
    diagnostics: Vec<ResolveDiagnostic>,
}

impl ResolvedValue {
    /// The effective typed value.
    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// The value's source layer.
    #[must_use]
    pub fn source(&self) -> &'static str {
        self.source
    }

    /// Lower-priority or ignored layer values.
    #[must_use]
    pub fn overridden(&self) -> &[(&'static str, Value)] {
        &self.overridden
    }
}

impl ResolvedSettings {
    /// The resolved value for `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Result<ResolvedValue, ResolveDiagnostic>> {
        self.values.get(key)
    }

    /// The effective text value of `key`.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)?.as_ref().ok()?.value() {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The effective integer value of `key`.
    #[must_use]
    pub fn integer(&self, key: &str) -> Option<i64> {
        match self.get(key)?.as_ref().ok()?.value() {
            Value::Integer(integer) => Some(*integer),
            _ => None,
        }
    }

    /// All diagnostics, including ignored widening requests.
    #[must_use]
    pub fn diagnostics(&self) -> &[ResolveDiagnostic] {
        &self.diagnostics
    }
}

/// Resolves all S1 settings without using storage, parsing, or UI adapters.
#[must_use]
pub fn resolve(
    registry: &Registry,
    classes: &SettingClasses,
    layers: &Resolved<'_>,
) -> ResolvedSettings {
    let mut result = ResolvedSettings::default();
    for descriptor in registry.descriptors() {
        let key = descriptor.key.as_ref();
        let class = classes.get(key);
        let candidates = layers.get(key).map(candidates).unwrap_or_default();
        let default = registry.default_of(key).cloned();
        let resolved = match (class, default) {
            (Some(SettingClass::Locked), Some(value)) => locked(key, value, &candidates),
            (Some(SettingClass::Bounded), Some(default)) => {
                bounded(key, default, candidates, &mut result.diagnostics)
            }
            (Some(SettingClass::Additive), Some(default)) => additive(key, default, candidates),
            (Some(SettingClass::Free), Some(default)) => Ok(first(default, candidates)),
            (None, _) => Err(diagnostic(key, "setting has no override class")),
            (_, None) => Err(diagnostic(key, "setting has no built-in default")),
        };
        result.values.insert(key.to_owned(), resolved);
    }
    result
}

/// Recovers supplied values from S1 resolution without rereading settings.
fn candidates(setting: &ResolvedSetting<'_>) -> Vec<(&'static str, Value)> {
    let mut result = Vec::new();
    if let Some(source) = source_layer(&setting.source) {
        result.push((source, setting.value.clone()));
    }
    result.extend(
        setting
            .overridden
            .iter()
            .filter_map(|(source, value)| source_layer(source).map(|layer| (layer, value.clone()))),
    );
    result
}

/// Maps S1 provenance names to the restriction adapter's layer labels.
fn source_layer(source: &Source) -> Option<&'static str> {
    match source {
        Source::Flag => Some("flag"),
        Source::File {
            layer: LayerName::Project,
            ..
        } => Some("workspace"),
        Source::File {
            layer: LayerName::User,
            ..
        } => Some("user"),
        Source::Default => None,
    }
}

/// Applies ordinary first-layer-wins precedence.
fn first(default: Value, candidates: Vec<(&'static str, Value)>) -> ResolvedValue {
    let mut candidates = candidates.into_iter();
    let (source, value) = candidates.next().unwrap_or(("default", default));
    ResolvedValue {
        value,
        source,
        overridden: candidates.collect(),
    }
}

/// Refuses any mutation of a locked value.
fn locked(
    key: &str,
    default: Value,
    candidates: &[(&'static str, Value)],
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if candidates.is_empty() {
        return Ok(ResolvedValue {
            value: default,
            source: "default",
            overridden: Vec::new(),
        });
    }
    Err(diagnostic(
        key,
        "locked setting cannot be changed by a preference layer",
    ))
}

/// Applies update ceilings, permission intersection, and strictest numeric bounds.
fn bounded(
    key: &str,
    default: Value,
    candidates: Vec<(&'static str, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if key == "updates" {
        return update_ceiling(key, default, candidates, diagnostics);
    }
    if key == "model_profile" {
        return profile_ceiling(key, default, candidates, diagnostics);
    }
    if candidates.is_empty() {
        return Ok(ResolvedValue {
            value: default,
            source: "default",
            overridden: Vec::new(),
        });
    }
    let default_value = default.clone();
    let mut values = candidates;
    if let Value::Integer(integer) = default {
        values.push(("default", Value::Integer(integer)));
    }
    if let Value::Flag(flag) = default_value {
        values.push(("default", Value::Flag(flag)));
    }
    if let Some((_, Value::Flag(_))) = values.first() {
        let mut chosen = 0;
        for (index, (_, candidate)) in values.iter().enumerate() {
            if let Value::Flag(false) = candidate {
                chosen = index;
                break;
            }
        }
        let (source, value) = values.remove(chosen);
        if let Value::Flag(false) = value {
            record_permission_widenings(key, &values, diagnostics);
        }
        return Ok(ResolvedValue {
            value,
            source,
            overridden: values,
        });
    }
    if let Some((_, Value::Integer(_))) = values.first() {
        let mut chosen = 0;
        for (index, (_, candidate)) in values.iter().enumerate().skip(1) {
            if let Value::Integer(value) = candidate
                && let Some((_, Value::Integer(best))) = values.get(chosen)
                && value < best
            {
                chosen = index;
            }
        }
        let (source, value) = values.remove(chosen);
        if let Value::Integer(bound) = value {
            record_budget_widenings(key, &values, bound, diagnostics);
            return Ok(ResolvedValue {
                value: Value::Integer(bound),
                source,
                overridden: values,
            });
        }
        return Ok(ResolvedValue {
            value,
            source,
            overridden: values,
        });
    }
    let (source, value) = values.remove(0);
    Ok(ResolvedValue {
        value,
        source,
        overridden: values,
    })
}

/// Records permission values that widen a selected false setting.
fn record_permission_widenings(
    key: &str,
    values: &[(&'static str, Value)],
    diagnostics: &mut Vec<ResolveDiagnostic>,
) {
    for (layer, value) in values {
        if flag_widens_permission(value) {
            diagnostics.push(diagnostic(
                key,
                format!("ignored {layer} permission widening"),
            ));
        }
    }
}

/// Whether `value` would enable a bounded capability.
fn flag_widens_permission(value: &Value) -> bool {
    value == &Value::Flag(true)
}

/// Records non-default layers that request more of an integer budget than `bound`.
fn record_budget_widenings(
    key: &str,
    values: &[(&'static str, Value)],
    bound: i64,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) {
    for (layer, value) in values {
        if integer_widens_budget(value, bound, layer) {
            diagnostics.push(diagnostic(key, format!("ignored {layer} budget widening")));
        }
    }
}

/// Whether a non-default layer requests more of a numeric budget than `bound`.
fn integer_widens_budget(value: &Value, bound: i64, layer: &str) -> bool {
    match value {
        Value::Integer(requested) => *requested > bound && layer != "default",
        _ => false,
    }
}

/// Restricts updates to the user/default ceiling and diagnoses attempted widening.
fn update_ceiling(
    key: &str,
    default: Value,
    candidates: Vec<(&'static str, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    let Value::Text(default) = default else {
        return Err(diagnostic(key, "updates default is not text"));
    };
    let user = candidates.iter().find(|(source, _)| *source == "user");
    let ceiling = user.map_or(default.as_str(), |(_, value)| match value {
        Value::Text(value) => value.as_str(),
        _ => default.as_str(),
    });
    let mut chosen = ceiling.to_owned();
    let mut source = user.map_or("default", |(source, _)| *source);
    for (layer, value) in candidates {
        if layer == "user" {
            continue;
        }
        let Value::Text(value) = value else {
            diagnostics.push(diagnostic(key, "update value is not text"));
            continue;
        };
        if value == "auto" {
            diagnostics.push(diagnostic(key, format!("ignored {layer} auto widening")));
        } else if value == "off" {
            "off".clone_into(&mut chosen);
            source = layer;
        } else if value == "propose" && chosen == "auto" {
            "propose".clone_into(&mut chosen);
            source = layer;
        }
    }
    Ok(ResolvedValue {
        value: Value::Text(chosen),
        source,
        overridden: Vec::new(),
    })
}

/// Restricts workspace and flag model profiles to the user/default ceiling.
fn profile_ceiling(
    key: &str,
    default: Value,
    candidates: Vec<(&'static str, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    let Value::Text(default) = default else {
        return Err(diagnostic(key, "model profile default is not text"));
    };
    let mut ceiling = default;
    let mut source = "default";
    for (layer, value) in &candidates {
        if *layer == "user"
            && let Value::Text(value) = value
        {
            ceiling.clone_from(value);
            source = layer;
        }
    }
    let mut chosen = ceiling.clone();
    for (layer, value) in candidates {
        if layer == "user" {
            continue;
        }
        let Value::Text(value) = value else {
            diagnostics.push(diagnostic(key, "model profile value is not text"));
            continue;
        };
        if profile_rank(&value) <= profile_rank(&chosen) {
            chosen = value;
            source = layer;
        } else {
            diagnostics.push(diagnostic(key, format!("ignored {layer} profile widening")));
        }
    }
    Ok(ResolvedValue {
        value: Value::Text(chosen),
        source,
        overridden: Vec::new(),
    })
}

/// Orders the three supported profiles from narrowest to broadest.
fn profile_rank(profile: &str) -> u8 {
    match profile {
        "fast" => 0,
        "balanced" => 1,
        "deep" => 2,
        _ => 3,
    }
}

/// Accumulates additive flags, lists and prose across all supplied layers.
fn additive(
    key: &str,
    default: Value,
    candidates: Vec<(&'static str, Value)>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if candidates.is_empty() {
        return Ok(ResolvedValue {
            value: default,
            source: "default",
            overridden: Vec::new(),
        });
    }
    let mut value = default;
    for (_, candidate) in &candidates {
        match (&mut value, candidate) {
            (Value::Flag(current), Value::Flag(next)) => *current |= *next,
            (Value::List(current), Value::List(next)) => merge_list(current, next),
            (Value::Text(current), Value::Text(next)) => {
                if !current.is_empty() && !next.is_empty() {
                    current.push('\n');
                }
                current.push_str(next);
            }
            _ => return Err(diagnostic(key, "layer values cannot be accumulated")),
        }
    }
    Ok(ResolvedValue {
        value,
        source: "combined",
        overridden: candidates,
    })
}

/// Appends unseen values in layer order so list accumulation is deterministic.
fn merge_list(current: &mut Vec<String>, next: &[String]) {
    for item in next {
        if !current.contains(item) {
            current.push(item.clone());
        }
    }
}

/// Constructs a keyed resolution diagnostic.
fn diagnostic(key: &str, message: impl Into<String>) -> ResolveDiagnostic {
    ResolveDiagnostic {
        key: key.to_owned(),
        message: message.into(),
    }
}
