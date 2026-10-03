//! Standard values constrain the existing C17 resolver, never ordinary precedence.

use super::types::{Layer, ResolveDiagnostic, ResolvedValue, diagnostic, first};
use maestro_settings::{SettingClass, SettingDescriptor, SettingKind, Value};
use std::borrow::Cow;

/// Merge admitted standard values, then refuse every widening consumer candidate.
pub(super) fn constrained(
    descriptor: &SettingDescriptor,
    default: Option<Value>,
    first: &Value,
    remaining: &[Value],
    candidates: Vec<(Layer, Value)>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    let key = descriptor.key.as_ref();
    if !matches!(
        descriptor.class,
        SettingClass::Bounded | SettingClass::Additive
    ) {
        return Err(diagnostic(
            key,
            "standard requires a restrictive override class",
        ));
    }
    let mut baseline = if descriptor.class == SettingClass::Additive {
        additive(
            key,
            default.ok_or_else(|| diagnostic(key, "setting has no built-in default"))?,
            vec![(Layer::Standard, first.clone())],
        )?
        .value
    } else {
        first.clone()
    };
    for value in remaining {
        baseline = merge(descriptor, baseline, vec![(Layer::Standard, value.clone())])?
            .value()
            .clone();
    }
    for (_, value) in &candidates {
        if !narrows(descriptor, &baseline, value) {
            return Err(diagnostic(key, "request widens standard"));
        }
    }
    merge(descriptor, baseline, candidates)
}

/// Direction comes from the setting class and kind, never a product or rule ID.
fn narrows(descriptor: &SettingDescriptor, baseline: &Value, value: &Value) -> bool {
    match (descriptor.class, baseline, value) {
        (SettingClass::Additive, Value::Flag(true), Value::Flag(false)) => false,
        (SettingClass::Additive, _, _) | (_, Value::Off, _) => true,
        (_, Value::Flag(bound), Value::Flag(value)) => !value || *bound,
        (_, Value::Integer(bound), Value::Integer(value)) => numeric_narrows(bound, value, false),
        (_, Value::Number(bound), Value::Number(value)) => numeric_narrows(bound, value, false),

        (_, Value::List(bound), Value::List(value)) => {
            value.iter().all(|item| bound.contains(item))
        }
        (_, Value::Text(bound), Value::Text(value)) => {
            if let SettingKind::Choice {
                ordered: true,
                values,
                ..
            } = &descriptor.kind
            {
                let rank = |text: &str| values.iter().position(|item| item == text);
                rank(value) <= rank(bound)
            } else {
                value == bound
            }
        }
        _ => false,
    }
}

/// Invoke C17's existing accumulation or restriction with a standard ceiling.
fn merge(
    descriptor: &SettingDescriptor,
    baseline: Value,
    candidates: Vec<(Layer, Value)>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if descriptor.class == SettingClass::Additive {
        let mut resolved = additive(&descriptor.key, baseline, candidates)?;
        if resolved.source == Layer::Default {
            resolved.source = Layer::Standard;
        }
        return Ok(resolved);
    }
    if let Value::Text(bound) = &baseline
        && !matches!(&descriptor.kind, SettingKind::Choice { ordered: true, .. })
        && candidates
            .iter()
            .any(|(_, value)| value != &Value::Text(bound.clone()))
    {
        return Err(diagnostic(
            &descriptor.key,
            "conflicting standard constraints",
        ));
    }
    let standard_only = candidates.is_empty();
    let mut resolved = bounded(
        &descriptor.key,
        &descriptor.kind,
        baseline,
        candidates,
        &mut Vec::new(),
    )?;
    if resolved.source == Layer::Default || standard_only {
        resolved.source = Layer::Standard;
    }
    Ok(resolved)
}

/// Intersect list permissions; report attempted additions as other C17 bounds do.
pub(super) fn list(
    key: &str,
    default: Vec<String>,
    candidates: Vec<(Layer, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> ResolvedValue {
    let mut values = candidates
        .iter()
        .find_map(|(layer, value)| {
            if *layer == Layer::User
                && let Value::List(values) = value
            {
                Some(values.clone())
            } else {
                None
            }
        })
        .unwrap_or(default);
    for (layer, value) in &candidates {
        if *layer == Layer::User {
            continue;
        }
        if let Value::List(requested) = value {
            if requested.iter().any(|item| !values.contains(item)) {
                diagnostics.push(diagnostic(key, "ignored permission list widening"));
            }
            values.retain(|item| requested.contains(item));
        }
    }
    chosen(Value::List(values), Layer::Combined, candidates)
}

/// Applies the declared ordered choice, permission, and numeric ceiling rules.
pub(super) fn bounded(
    key: &str,
    kind: &SettingKind,
    default: Value,
    candidates: Vec<(Layer, Value)>,
    diagnostics: &mut Vec<ResolveDiagnostic>,
) -> Result<ResolvedValue, ResolveDiagnostic> {
    if let SettingKind::Choice {
        ordered: true,
        values,
        ..
    } = kind
    {
        return ordered_choice(key, values, default, candidates, diagnostics);
    }
    match default {
        Value::Flag(default) => Ok(bounded_flag(key, default, candidates, diagnostics)),
        Value::Integer(_) | Value::Number(_) | Value::Off => {
            Ok(bounded_numeric(key, default, candidates, diagnostics))
        }
        Value::List(values) => Ok(list(key, values, candidates, diagnostics)),
        value @ Value::Text(_) => Ok(first(value, candidates)),
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
        Value::Integer(_) | Value::Number(_) | Value::Off => Some(value.clone()),
        _ => None,
    });
    let ceiling = selected.0.clone();
    let mut selected = selected;
    for (layer, value) in &candidates {
        match (&selected.0, value) {
            (Value::Off, Value::Integer(_) | Value::Number(_)) => {
                selected = (value.clone(), *layer);
            }
            (Value::Integer(bound), Value::Integer(requested)) if requested < bound => {
                selected = (Value::Integer(*requested), *layer);
            }
            (Value::Number(bound), Value::Number(requested)) if requested < bound => {
                selected = (value.clone(), *layer);
            }
            (Value::Integer(_) | Value::Number(_), Value::Off) if *layer != Layer::User => {
                diagnostics.push(diagnostic(
                    key,
                    format!("ignored {} budget widening", layer.name()),
                ));
            }
            (Value::Integer(bound), Value::Integer(requested))
                if *layer != Layer::User
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
            (Value::Number(bound), Value::Number(requested))
                if *layer != Layer::User
                    && (if optional {
                        requested > bound
                    } else {
                        matches!(&ceiling, Value::Number(limit) if requested > limit)
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
pub(super) fn chosen(
    value: Value,
    source: Layer,
    candidates: Vec<(Layer, Value)>,
) -> ResolvedValue {
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
pub(crate) fn additive(
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

/// Shared numeric restriction direction: floors rise, ceilings fall.
pub(crate) fn numeric_narrows<T: PartialOrd>(baseline: &T, value: &T, floor: bool) -> bool {
    if floor {
        value >= baseline
    } else {
        value <= baseline
    }
}
