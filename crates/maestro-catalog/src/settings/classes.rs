//! Checked catalog override classes for the canonical S1 setting descriptors.

use maestro_settings::Registry;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error, fmt,
};

/// One of the catalog's override classes from architecture 03 §1.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingClass {
    /// Explicit flags, workspace, user and defaults precedence.
    Free,
    /// A value that can only narrow an existing ceiling.
    Bounded,
    /// A value that can only add restrictions or checks.
    Additive,
    /// A value no preference layer may change.
    Locked,
}

/// The strict TOML document containing the catalog class lists.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassDocument {
    /// Keys grouped under each one of the four classes.
    classes: BTreeMap<String, Vec<String>>,
}

/// The checked class of every key in S1's registry.
#[derive(Debug, Clone, Default)]
pub struct SettingClasses {
    /// The unique checked class for each S1 registry key.
    by_key: BTreeMap<String, SettingClass>,
}

/// Why a catalog class table is incomplete, duplicated or names an unknown key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingClassError(String);

impl SettingClasses {
    /// Parses a catalog class table and proves it covers S1's registry exactly once.
    ///
    /// # Errors
    ///
    /// Returns [`SettingClassError`] for malformed TOML, unknown classes or
    /// keys, duplicate classification, or a missing S1 key.
    pub fn parse(text: &str, registry: &Registry) -> Result<Self, SettingClassError> {
        let document: ClassDocument = toml::from_str(text)
            .map_err(|error| SettingClassError(format!("invalid class table: {error}")))?;
        let known: BTreeSet<&str> = registry
            .descriptors()
            .map(|descriptor| descriptor.key.as_ref())
            .collect();
        let mut by_key = BTreeMap::new();
        for (class_name, keys) in document.classes {
            let class = parse_class(&class_name)?;
            for key in keys {
                insert_key(&mut by_key, &known, class, &key)?;
            }
        }
        if let Some(key) = known.into_iter().find(|key| !by_key.contains_key(*key)) {
            return Err(SettingClassError(format!("setting {key:?} has no class")));
        }
        Ok(Self { by_key })
    }

    /// The class declared for `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<SettingClass> {
        self.by_key.get(key).copied()
    }

    /// The canonical S1 setting keys consumed by this checked catalog table.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.by_key.keys().map(String::as_str)
    }
}

/// Converts the TOML table name to its class enum.
fn parse_class(name: &str) -> Result<SettingClass, SettingClassError> {
    match name {
        "free" => Ok(SettingClass::Free),
        "bounded" => Ok(SettingClass::Bounded),
        "additive" => Ok(SettingClass::Additive),
        "locked" => Ok(SettingClass::Locked),
        other => Err(SettingClassError(format!(
            "unknown override class {other:?}"
        ))),
    }
}

/// Rejects a key outside S1's set or a second class entry.
fn insert_key(
    by_key: &mut BTreeMap<String, SettingClass>,
    known: &BTreeSet<&str>,
    class: SettingClass,
    key: &str,
) -> Result<(), SettingClassError> {
    if !known.contains(key) {
        return Err(SettingClassError(format!("unknown setting {key:?}")));
    }
    if by_key.insert(key.to_owned(), class).is_some() {
        return Err(SettingClassError(format!(
            "setting {key:?} has two classes"
        )));
    }
    Ok(())
}

impl fmt::Display for SettingClassError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl error::Error for SettingClassError {}
