//! Typed restrictive resolution over four preference layers; storage and parsing stay in S1.

use maestro_settings::{
    LayerName, Registry, Resolved, ResolvedSetting, SettingClass, SettingKind, Source, Value,
};
use std::{borrow::Cow, collections::BTreeMap};

/// A chosen value with its source and values it overrode.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedValue {
    /// The effective typed value.
    value: Value,
    /// The layer that supplied or constrained the value.
    source: Layer,
    /// Every supplied value other than the effective candidate.
    overridden: Vec<(Layer, Value)>,
}

/// The four accepted preference sources, in precedence order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Explicit command-line value.
    /// Explicit command-line value.
    Flag,
    /// Workspace configuration file.
    Workspace,
    /// User configuration file.
    User,
    /// Built-in descriptor default.
    Default,
    /// Accumulated additive value.
    Combined,
}

impl Layer {
    /// Stable display name for provenance.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Workspace => "workspace",
            Self::User => "user",
            Self::Default => "default",
            Self::Combined => "combined",
        }
    }
}

/// A setting could not be resolved because it was locked or invalid.
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
        self.source.name()
    }

    /// Lower-priority or ignored layer values.
    #[must_use]
    pub fn overridden(&self) -> &[(Layer, Value)] {
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

    /// Ignored widening requests; refusals are returned by `get`.
    #[must_use]
    pub fn diagnostics(&self) -> &[ResolveDiagnostic] {
        &self.diagnostics
    }
}

/// Resolves all S1 settings without using storage, parsing, or UI adapters.
#[must_use]
pub fn resolve(registry: &Registry, layers: &Resolved<'_>) -> ResolvedSettings {
    let mut result = ResolvedSettings::default();
    for descriptor in registry.descriptors() {
        let key = descriptor.key.as_ref();
        let candidates = layers.get(key).map(candidates).unwrap_or_default();
        let default = registry.default_of(key).cloned();
        let resolved = match (descriptor.class, default) {
            (SettingClass::Locked, Some(value)) => locked(key, value, &candidates),
            (SettingClass::Bounded, Some(default)) => bounded(
                key,
                &descriptor.kind,
                default,
                candidates,
                &mut result.diagnostics,
            ),
            (SettingClass::Additive, Some(default)) => additive(key, default, candidates),
            (SettingClass::Free, Some(default)) => Ok(first(default, candidates)),
            (_, None) => Err(diagnostic(key, "setting has no built-in default")),
        };
        result.values.insert(key.to_owned(), resolved);
    }
    result
}

/// Recovers supplied values from S1 resolution without rereading settings.
fn candidates(setting: &ResolvedSetting<'_>) -> Vec<(Layer, Value)> {
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

/// Maps S1 provenance to a typed restriction layer.
fn source_layer(source: &Source) -> Option<Layer> {
    match source {
        Source::Flag => Some(Layer::Flag),
        Source::File {
            layer: LayerName::Project,
            ..
        } => Some(Layer::Workspace),
        Source::File {
            layer: LayerName::User,
            ..
        } => Some(Layer::User),
        Source::Default => None,
    }
}

/// Applies ordinary first-layer-wins precedence.
fn first(default: Value, candidates: Vec<(Layer, Value)>) -> ResolvedValue {
    let mut candidates = candidates.into_iter();
    let (source, value) = candidates.next().unwrap_or((Layer::Default, default));
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
    candidates: &[(Layer, Value)],
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if candidates.is_empty() {
        return Ok(ResolvedValue {
            value: default,
            source: Layer::Default,
            overridden: Vec::new(),
        });
    }
    Err(diagnostic(
        key,
        "locked setting cannot be changed by a preference layer",
    ))
}

/// Applies the declared ordered choice, permission, and numeric ceiling rules.
fn bounded(
    key: &str,
    kind: &SettingKind,
    default: Value,
    candidates: Vec<(Layer, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if let SettingKind::Choice { values, .. } = kind
        && values
            .iter()
            .map(Cow::as_ref)
            .eq(["off", "propose", "auto"])
    {
        return ordered_choice(key, values, default, candidates, diagnostics);
    }
    match default {
        Value::Flag(default) => Ok(bounded_flag(key, default, candidates, diagnostics)),
        Value::Integer(_) | Value::Off => {
            Ok(bounded_numeric(key, default, candidates, diagnostics))
        }
        value => Ok(first(value, candidates)),
    }
}

/// Returns the user's decoded value or the default with its source layer.
fn user_ceiling<T>(
    candidates: &[(Layer, Value)],
    default: T,
    decode: impl FnOnce(&Value) -> Option<T>,
) -> (T, Layer) {
    candidates
        .iter()
        .find(|(layer, _)| *layer == Layer::User)
        .and_then(|(_, value)| decode(value).map(|value| (value, Layer::User)))
        .unwrap_or((default, Layer::Default))
}

/// Uses the user's value (or default) as ceiling; other layers only narrow it.
fn bounded_flag(
    key: &str,
    default: bool,
    candidates: Vec<(Layer, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> ResolvedValue {
    let ceiling = user_ceiling(&candidates, default, |value| match value {
        Value::Flag(value) => Some(*value),
        _ => None,
    });
    let narrowing = candidates
        .iter()
        .find(|(layer, value)| *layer != Layer::User && value == &Value::Flag(false));
    let (source, value) = narrowing.map_or((ceiling.1, ceiling.0), |(layer, _)| (*layer, false));
    for (layer, candidate) in &candidates {
        if *layer != Layer::User && !ceiling.0 && candidate == &Value::Flag(true) {
            diagnostics.push(diagnostic(
                key,
                format!("ignored {} permission widening", layer.name()),
            ));
        }
    }
    chosen(Value::Flag(value), source, candidates)
}

/// Takes the minimum numeric value under the user's value or default ceiling.
fn bounded_numeric(
    key: &str,
    default: Value,
    candidates: Vec<(Layer, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> ResolvedValue {
    let optional = matches!(&default, Value::Off);
    let selected = user_ceiling(&candidates, default, |value| match value {
        Value::Integer(_) | Value::Off => Some(value.clone()),
        _ => None,
    });
    let ceiling = selected.0.clone();
    let mut selected = selected;
    for (layer, value) in &candidates {
        match (&selected.0, value) {
            (Value::Off, Value::Integer(_)) => selected = (value.clone(), *layer),
            (Value::Integer(bound), Value::Integer(requested)) if requested < bound => {
                selected = (Value::Integer(*requested), *layer);
            }
            (Value::Integer(_), Value::Off) if *layer != Layer::User => {
                diagnostics.push(diagnostic(
                    key,
                    format!("ignored {} budget widening", layer.name()),
                ));
            }
            (Value::Integer(bound), Value::Integer(requested))
                if (optional || *layer != Layer::User)
                    && (if optional {
                        requested > bound
                    } else {
                        matches!(&ceiling, Value::Integer(limit) if requested > limit)
                    }) =>
            {
                diagnostics.push(diagnostic(
                    key,
                    format!("ignored {} budget widening", layer.name()),
                ));
            }
            _ => {}
        }
    }
    chosen(selected.0, selected.1, candidates)
}

/// Applies a descriptor-declared ordering to updates and explains every widening.
fn ordered_choice(
    key: &str,
    values: &[Cow<'static, str>],
    default: Value,
    candidates: Vec<(Layer, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    let Value::Text(default_value) = default else {
        return Err(diagnostic(key, "ordered setting default is not text"));
    };
    let user_value = candidates.iter().find(|(layer, _)| *layer == Layer::User);
    let ceiling = user_value
        .and_then(|(_, value)| match value {
            Value::Text(value) => Some(value.as_str()),
            _ => None,
        })
        .unwrap_or(&default_value);
    let mut selected = (
        ceiling.to_owned(),
        user_value.map_or(Layer::Default, |(layer, _)| *layer),
    );
    let rank = |value: &str| {
        values
            .iter()
            .position(|item| item.as_ref() == value)
            .unwrap_or(usize::MAX)
    };
    for (layer, value) in &candidates {
        let Value::Text(value) = value else { continue };
        if *layer == Layer::User {
            continue;
        }
        if rank(value) < rank(&selected.0) {
            selected = (value.clone(), *layer);
        } else if rank(value) > rank(ceiling) {
            diagnostics.push(diagnostic(
                key,
                format!("ignored {} update widening", layer.name()),
            ));
        }
    }
    Ok(chosen(Value::Text(selected.0), selected.1, candidates))
}

/// Keeps all requested values except the candidate that supplied the result.
fn chosen(value: Value, source: Layer, candidates: Vec<(Layer, Value)>) -> ResolvedValue {
    let mut skipped_chosen = false;
    let overridden = candidates
        .into_iter()
        .filter(|candidate| {
            if !skipped_chosen && candidate.0 == source && candidate.1 == value {
                skipped_chosen = true;
                false
            } else {
                true
            }
        })
        .collect();
    ResolvedValue {
        value,
        source,
        overridden,
    }
}

/// Accumulates additive flags, lists and prose across all supplied layers.
fn additive(
    key: &str,
    mut value: Value,
    candidates: Vec<(Layer, Value)>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if candidates.is_empty() {
        return Ok(ResolvedValue {
            value,
            source: Layer::Default,
            overridden: Vec::new(),
        });
    }
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
        source: Layer::Combined,
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
