//! Resolution (S3 FR-S3-014, plan D6): each key takes the value of the
//! first layer that sets it, in the order explicit `--set` flag, project file,
//! user file, built-in default, and keeps the layers it overrode for
//! `config explain`. Every present file is parsed whole even when a flag
//! masks one of its keys, so a flag never hides a refused file.

use crate::{
    descriptor::{SettingClass, SettingDescriptor},
    layer::{LOCKED, Layer, LayerError},
    registry::Registry,
    value::Value,
};
use std::{error, fmt, path::PathBuf};

/// Which preferences file a value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerName {
    /// The user's `preferences.toml`, in the configuration directory.
    User,
    /// The project's `.maestro/config.toml`.
    Project,
}

impl LayerName {
    /// Its name, as `config explain` writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }
}

/// Where a setting's value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The registry's built-in default.
    Default,
    /// A preferences file.
    File {
        /// Which one.
        layer: LayerName,
        /// Its path.
        path: PathBuf,
    },
    /// An explicit `--set` flag.
    Flag,
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => formatter.write_str("default"),
            Self::File { layer, path } => {
                write!(formatter, "{} file {}", layer.name(), path.display())
            }
            Self::Flag => formatter.write_str("--set flag"),
        }
    }
}

/// The files a session reads, each with the layer it gave; a missing file
/// is `None`, an empty layer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layers {
    /// The user file.
    pub user: Option<(PathBuf, Layer)>,
    /// The project file.
    pub project: Option<(PathBuf, Layer)>,
}

/// One explicit `--set KEY=VALUE`, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Flag {
    /// The setting's key.
    pub key: String,
    /// Its value.
    pub value: Value,
}

/// The `--set KEY=VALUE` flags `texts`, each checked against `registry`.
///
/// # Errors
///
/// [`SettingsError::Flag`], quoting the flag, for text without `=`, an
/// unknown key, a value its setting refuses, a locked setting or a key set
/// twice.
pub fn parse_flags(registry: &Registry, texts: &[String]) -> Result<Vec<Flag>, SettingsError> {
    let mut flags: Vec<Flag> = Vec::with_capacity(texts.len());
    for text in texts {
        let refuse = |reason: String| SettingsError::Flag {
            text: text.clone(),
            reason,
        };
        let (key, value) = text
            .split_once('=')
            .ok_or_else(|| refuse("expected KEY=VALUE".to_owned()))?;
        let descriptor = registry
            .get(key)
            .ok_or_else(|| refuse(format!("unknown key {key:?}")))?;
        if descriptor.class == SettingClass::Locked {
            return Err(refuse(LOCKED.to_owned()));
        }
        let value = descriptor
            .kind
            .parse_text(value)
            .map_err(|error| refuse(error.to_string()))?;
        if flags.iter().any(|flag| flag.key == key) {
            return Err(refuse(format!("{key} is set twice")));
        }
        flags.push(Flag {
            key: key.to_owned(),
            value,
        });
    }
    Ok(flags)
}

/// One setting's effective value.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSetting<'a> {
    /// Its descriptor.
    pub descriptor: &'a SettingDescriptor,
    /// Its effective value.
    pub value: Value,
    /// The layer that set it.
    pub source: Source,
    /// The layers below it that set it too, highest first, with their values.
    pub overridden: Vec<(Source, Value)>,
}

/// Every setting's effective value, in registry order.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved<'a>(Vec<ResolvedSetting<'a>>);

/// Resolves every setting of `registry` from `layers` and `flags`.
#[must_use]
pub fn resolve<'a>(registry: &'a Registry, layers: &Layers, flags: &[Flag]) -> Resolved<'a> {
    let files = [
        (LayerName::Project, layers.project.as_ref()),
        (LayerName::User, layers.user.as_ref()),
    ];
    let settings = registry
        .descriptors()
        .map(|descriptor| {
            let key: &str = &descriptor.key;
            let mut found: Vec<(Source, Value)> = flags
                .iter()
                .filter(|flag| flag.key == key)
                .map(|flag| (Source::Flag, flag.value.clone()))
                .collect();
            for (layer, file) in files {
                if let Some((path, values)) = file
                    && let Some(value) = values.get(key)
                {
                    let source = Source::File {
                        layer,
                        path: path.clone(),
                    };
                    found.push((source, value.clone()));
                }
            }
            let default = registry.default_of(key).cloned().unwrap_or(Value::Off);
            let mut found = found.into_iter();
            let (source, value) = found.next().unwrap_or((Source::Default, default));
            ResolvedSetting {
                descriptor,
                value,
                source,
                overridden: found.collect(),
            }
        })
        .collect();
    Resolved(settings)
}

impl<'a> Resolved<'a> {
    /// Every setting, in registry order.
    pub fn iter(&self) -> impl Iterator<Item = &ResolvedSetting<'a>> {
        self.0.iter()
    }

    /// The setting of `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ResolvedSetting<'a>> {
        self.0.iter().find(|setting| setting.descriptor.key == key)
    }

    /// The effective value of `key`.
    #[must_use]
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.get(key).map(|setting| &setting.value)
    }

    /// The flag `key` holds, when it is a flag.
    #[must_use]
    pub fn flag(&self, key: &str) -> Option<bool> {
        match self.value(key) {
            Some(Value::Flag(flag)) => Some(*flag),
            _ => None,
        }
    }

    /// The whole number `key` holds, when it holds one.
    #[must_use]
    pub fn integer(&self, key: &str) -> Option<i64> {
        match self.value(key) {
            Some(Value::Integer(integer)) => Some(*integer),
            _ => None,
        }
    }

    /// The number `key` holds, when it holds one.
    #[must_use]
    pub fn number(&self, key: &str) -> Option<f64> {
        match self.value(key) {
            Some(Value::Number(number)) => Some(*number),
            _ => None,
        }
    }

    /// The text `key` holds, when it holds one.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.value(key) {
            Some(Value::Text(text)) => Some(text),
            _ => None,
        }
    }

    /// The list `key` holds, when it holds one.
    #[must_use]
    pub fn list(&self, key: &str) -> Option<&[String]> {
        match self.value(key) {
            Some(Value::List(items)) => Some(items),
            _ => None,
        }
    }

    /// Whether `key` holds `off`.
    #[must_use]
    pub fn is_off(&self, key: &str) -> bool {
        matches!(self.value(key), Some(Value::Off))
    }
}

/// Why a session's settings could not be resolved.
#[derive(Debug)]
pub enum SettingsError {
    /// A preferences file was refused.
    File {
        /// The file.
        path: PathBuf,
        /// Why.
        error: LayerError,
    },
    /// A preferences file exists but cannot be read.
    Io {
        /// The file.
        path: PathBuf,
        /// What the operating system reported.
        reason: String,
    },
    /// A `--set` flag was refused.
    Flag {
        /// The flag's text.
        text: String,
        /// Why.
        reason: String,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path, error } => write!(formatter, "{}: {error}", path.display()),
            Self::Io { path, reason } => {
                write!(formatter, "cannot read {}: {reason}", path.display())
            }
            Self::Flag { text, reason } => write!(formatter, "--set {text}: {reason}"),
        }
    }
}

impl error::Error for SettingsError {}
