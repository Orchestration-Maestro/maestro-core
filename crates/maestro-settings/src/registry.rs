//! The registry: the descriptors of every setting Maestro knows, each checked
//! once when the registry is built, with its default parsed. Everything else
//! (files, flags, resolution, editing, explanations) reads settings through
//! it, so a new descriptor is a new setting everywhere.

use crate::{
    builtin::BUILT_IN,
    descriptor::{SettingDescriptor, SettingKind, Text},
    value::Value,
};
use std::{error, fmt};

/// The key a preferences file names its schema with, which no setting takes.
pub const SCHEMA_KEY: &str = "schema";

/// A checked set of descriptors, in declaration order.
#[derive(Debug, Clone)]
pub struct Registry {
    /// Each descriptor, with its parsed default.
    entries: Vec<(SettingDescriptor, Value)>,
}

impl Registry {
    /// The registry of `descriptors`.
    ///
    /// # Errors
    ///
    /// [`RegistryError`], naming the key, for a malformed or repeated key, a
    /// key that is also another's table, a kind whose range or values cannot
    /// hold, a default its kind refuses, or a class the resolver does not
    /// resolve yet.
    pub fn new(descriptors: &[SettingDescriptor]) -> Result<Self, RegistryError> {
        let mut entries: Vec<(SettingDescriptor, Value)> = Vec::with_capacity(descriptors.len());
        for descriptor in descriptors {
            let refuse = |reason: String| RegistryError {
                key: descriptor.key.to_string(),
                reason,
            };
            check_descriptor(descriptor).map_err(refuse)?;
            for (other, _) in &entries {
                check_pair(other, descriptor)?;
            }
            let default = descriptor
                .kind
                .parse_text(&descriptor.default)
                .map_err(|error| {
                    refuse(format!(
                        "its default {:?} is refused: {error}",
                        descriptor.default
                    ))
                })?;
            entries.push((descriptor.clone(), default));
        }
        Ok(Self { entries })
    }

    /// The registry of the settings Maestro ships, [`BUILT_IN`].
    ///
    /// # Errors
    ///
    /// As [`Registry::new`]; a test proves the built-in descriptors valid.
    pub fn built_in() -> Result<Self, RegistryError> {
        Self::new(BUILT_IN)
    }

    /// Every descriptor, in declaration order.
    #[must_use = "the descriptors are only read"]
    pub fn descriptors(&self) -> impl ExactSizeIterator<Item = &SettingDescriptor> {
        self.entries.iter().map(|(descriptor, _)| descriptor)
    }

    /// The descriptor of `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&SettingDescriptor> {
        self.entries
            .iter()
            .find(|(descriptor, _)| descriptor.key == key)
            .map(|(descriptor, _)| descriptor)
    }

    /// The parsed default of `key`.
    #[must_use]
    pub fn default_of(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(descriptor, _)| descriptor.key == key)
            .map(|(_, default)| default)
    }

    /// Whether `path` is a table: the dotted prefix of a key.
    #[must_use]
    pub fn is_table(&self, path: &str) -> bool {
        self.entries
            .iter()
            .any(|(descriptor, _)| nested(path, &descriptor.key).is_some())
    }
}

/// `(table, key)` when one of `first` and `second` is the table of the
/// other, the table first.
fn nested<'k>(first: &'k str, second: &'k str) -> Option<(&'k str, &'k str)> {
    let under = |table: &str, key: &str| {
        !table.is_empty()
            && key
                .strip_prefix(table)
                .is_some_and(|rest| rest.starts_with('.'))
    };
    if under(first, second) {
        Some((first, second))
    } else if under(second, first) {
        Some((second, first))
    } else {
        None
    }
}

/// Refuses `descriptor` when `earlier`, declared before it, has its key or
/// is its table, or the other way round.
fn check_pair(
    earlier: &SettingDescriptor,
    descriptor: &SettingDescriptor,
) -> Result<(), RegistryError> {
    if earlier.key == descriptor.key {
        return Err(RegistryError {
            key: descriptor.key.to_string(),
            reason: "declared twice".to_owned(),
        });
    }
    match nested(&earlier.key, &descriptor.key) {
        Some((table, key)) => Err(RegistryError {
            key: table.to_owned(),
            reason: format!("a key cannot also be the table of {key:?}"),
        }),
        None => Ok(()),
    }
}

/// Refuses a descriptor whose key, kind or class cannot be used.
fn check_descriptor(descriptor: &SettingDescriptor) -> Result<(), String> {
    if !is_key(&descriptor.key) {
        return Err(
            "a key is dotted lower-case segments of letters, digits and '_', each \
                    starting with a letter, and never `schema`"
                .to_owned(),
        );
    }
    let sound = match &descriptor.kind {
        SettingKind::Integer { min, max, .. } => min <= max,
        SettingKind::Number { min, max, .. } => min.is_finite() && max.is_finite() && min <= max,
        SettingKind::Choice { values, reserved } => {
            let names: Vec<Text> = reserved
                .iter()
                .map(|reserved| reserved.value.clone())
                .collect();
            let all: Vec<Text> = values.iter().chain(&names).cloned().collect();
            return check_values(values).and_then(|()| check_values(&all));
        }
        SettingKind::ChoiceList { values } => return check_values(values),
        SettingKind::Flag | SettingKind::Language | SettingKind::Name => true,
    };
    if sound {
        Ok(())
    } else {
        Err("its range is empty or not finite".to_owned())
    }
}

/// Refuses choice values that are empty, repeated (a reserved value among
/// them) or hold a comma, which the command line's lists could not tell
/// apart.
fn check_values(values: &[Text]) -> Result<(), String> {
    let sound = !values.is_empty()
        && values.iter().enumerate().all(|(index, value)| {
            !value.is_empty()
                && !value.contains(',')
                && !values.iter().take(index).any(|earlier| earlier == value)
        });
    if sound {
        Ok(())
    } else {
        Err("its values are empty, repeated, or hold a comma".to_owned())
    }
}

/// Whether `key` is dotted lower-case segments of letters, digits and `_`,
/// each starting with a letter, and not [`SCHEMA_KEY`].
fn is_key(key: &str) -> bool {
    key != SCHEMA_KEY
        && key.split('.').all(|segment| {
            segment
                .bytes()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

/// A descriptor the registry refused, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError {
    /// The key of the refused descriptor.
    pub key: String,
    /// Why it was refused.
    pub reason: String,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "setting {:?}: {}", self.key, self.reason)
    }
}

impl error::Error for RegistryError {}
