//! One preferences file, `maestro-preferences/1`, parsed strictly (plan D6,
//! ADR-0014): the schema marker is required, every key must be a registered
//! setting or one of their tables, every value must be of its setting's kind,
//! and a locked setting may not be set. A refusal names the dotted key.
//! TOML refuses a key given twice, and the reader a setting given twice in
//! two forms, such as a quoted dotted key and a table. The file is bounded
//! before it is parsed.

use crate::{
    descriptor::SettingClass,
    registry::{Registry, SCHEMA_KEY},
    value::Value,
};
use std::{
    collections::{BTreeMap, btree_map},
    error, fmt,
    ops::Range,
};
use toml::{
    Spanned,
    de::{DeTable, DeValue},
};

/// The schema a preferences file names with `schema = "..."`.
pub const SCHEMA: &str = "maestro-preferences/1";

/// Why a locked setting is refused wherever it is set.
pub(crate) const LOCKED: &str = "the setting is locked: no file or flag may change it";

/// The most bytes a preferences file may hold.
pub const MAX_FILE_BYTES: usize = 64 * 1024;

/// Default preferences container depth, counting the root as one.
pub const MAX_FILE_DEPTH: usize = 32;

/// The settings one file sets, by key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layer(BTreeMap<String, Value>);

impl Layer {
    /// The settings the preferences file `text` sets, checked against
    /// `registry`.
    ///
    /// # Errors
    ///
    /// [`LayerError`] for text over [`MAX_FILE_BYTES`], text that is not
    /// TOML, a missing or other schema, an unknown key, a value of another
    /// kind, or a locked setting.
    pub fn parse(registry: &Registry, text: &str) -> Result<Self, LayerError> {
        Self::read(registry, text, MAX_FILE_BYTES as u64, usize::MAX, false)
    }

    /// Parse a preferences file, accepting `[overrides]` over the same registry.
    /// Bytes are bounded before TOML parsing; container depth (root is one) is
    /// checked before interpreting settings, with TOML's own recursion cap retained.
    ///
    /// # Errors
    /// Returns a schema, registry, duplicate, size or depth refusal.
    pub fn parse_preferences(
        registry: &Registry,
        text: &str,
        max_bytes: u64,
        max_depth: usize,
    ) -> Result<Self, LayerError> {
        Self::read(registry, text, max_bytes, max_depth, true)
    }

    /// One parser for the legacy form and its additive overrides namespace.
    fn read(
        registry: &Registry,
        text: &str,
        max_bytes: u64,
        max_depth: usize,
        overrides: bool,
    ) -> Result<Self, LayerError> {
        if text.len() as u64 > max_bytes {
            return Err(if max_bytes == MAX_FILE_BYTES as u64 {
                LayerError::TooLarge(text.len())
            } else {
                LayerError::ByteLimit {
                    bytes: text.len(),
                    limit: max_bytes,
                }
            });
        }
        let document =
            DeTable::parse(text).map_err(|error| LayerError::NotToml(error.to_string()))?;
        let root = document.get_ref();
        check_depth(root, max_depth)?;
        let schema = root
            .iter()
            .find(|(key, _)| key.get_ref() == SCHEMA_KEY)
            .map(|(_, value)| value)
            .ok_or(LayerError::NoSchema)?;
        if !matches!(schema.get_ref(), DeValue::String(name) if name == SCHEMA) {
            return Err(LayerError::Refused {
                key: SCHEMA_KEY.to_owned(),
                reason: format!("expected {SCHEMA:?}"),
                found: source(text, schema.span()),
            });
        }
        let mut layer = Self::default();
        let reader = Reader {
            registry,
            text,
            strict_tables: overrides,
        };
        for (key, value) in root {
            if overrides && key.get_ref() == "overrides" {
                reader.overrides(value, &mut layer.0)?;
            } else if key.get_ref() != SCHEMA_KEY {
                reader.read(key.get_ref(), value, &mut layer.0)?;
            }
        }
        Ok(layer)
    }

    /// The value the file sets for `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// Whether the file sets nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Each setting the file sets, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }

    /// The layer with `key` set to `value`, or removed when `value` is
    /// `None`: what an edit must read back as.
    #[must_use]
    pub fn with(&self, key: &str, value: Option<&Value>) -> Self {
        let mut changed = self.0.clone();
        match value {
            Some(value) => {
                changed.insert(key.to_owned(), value.clone());
            }
            None => {
                changed.remove(key);
            }
        }
        Self(changed)
    }
}

impl<'layer> IntoIterator for &'layer Layer {
    type Item = (&'layer String, &'layer Value);
    type IntoIter = btree_map::Iter<'layer, String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// What reading a file's entries needs.
struct Reader<'registry, 'text> {
    /// The registry the keys are checked against.
    registry: &'registry Registry,
    /// The file's text, for what a refusal found.
    text: &'text str,
    /// Only directional descriptor prefixes are tables in opt-in preferences.
    strict_tables: bool,
}

impl Reader<'_, '_> {
    /// Read a single namespace over existing registry keys, never a raw option map.
    fn overrides(
        &self,
        value: &Spanned<DeValue<'_>>,
        values: &mut BTreeMap<String, Value>,
    ) -> Result<(), LayerError> {
        let DeValue::Table(table) = value.get_ref() else {
            return Err(LayerError::Refused {
                key: "overrides".to_owned(),
                reason: "expected a table".to_owned(),
                found: source(self.text, value.span()),
            });
        };
        for (key, value) in table {
            self.read(key.get_ref(), value, values)?;
        }
        Ok(())
    }

    /// Reads the entry `path` = `value` into `values`, through its tables.
    fn read(
        &self,
        path: &str,
        value: &Spanned<DeValue<'_>>,
        values: &mut BTreeMap<String, Value>,
    ) -> Result<(), LayerError> {
        let refuse = |reason: String| LayerError::Refused {
            key: path.to_owned(),
            reason,
            found: source(self.text, value.span()),
        };
        if let Some(descriptor) = self.registry.get(path) {
            if descriptor.class == SettingClass::Locked {
                return Err(LayerError::Locked(path.to_owned()));
            }
            let read = descriptor
                .kind
                .parse_toml(value.get_ref())
                .map_err(|error| refuse(error.to_string()))?;
            if values.insert(path.to_owned(), read).is_some() {
                return Err(LayerError::Twice(path.to_owned()));
            }
            return Ok(());
        }
        let is_table = if self.strict_tables {
            self.registry.descriptors().any(|descriptor| {
                descriptor
                    .key
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with('.'))
            })
        } else {
            self.registry.is_table(path)
        };
        if !is_table {
            return Err(LayerError::UnknownKey(path.to_owned()));
        }
        let DeValue::Table(table) = value.get_ref() else {
            return Err(refuse("expected a table".to_owned()));
        };
        for (key, inner) in table {
            self.read(&format!("{path}.{}", key.get_ref()), inner, values)?;
        }
        Ok(())
    }
}

/// Check all container levels before registry validation, including arrays.
fn check_depth(root: &DeTable<'_>, limit: usize) -> Result<(), LayerError> {
    if limit == 0 {
        return Err(LayerError::DepthLimit(limit));
    }
    let mut pending: Vec<_> = root.iter().map(|(_, value)| (value.get_ref(), 1)).collect();
    while let Some((value, parent_depth)) = pending.pop() {
        let depth = parent_depth + 1;
        match value {
            DeValue::Table(table) => {
                if depth > limit {
                    return Err(LayerError::DepthLimit(limit));
                }
                pending.extend(table.iter().map(|(_, value)| (value.get_ref(), depth)));
            }
            DeValue::Array(items) => {
                if depth > limit {
                    return Err(LayerError::DepthLimit(limit));
                }
                pending.extend(items.iter().map(|value| (value.get_ref(), depth)));
            }
            _ => {}
        }
    }
    Ok(())
}

/// The text of `span` in `text`, what a refusal found.
fn source(text: &str, span: Range<usize>) -> String {
    text.get(span).unwrap_or_default().to_owned()
}

/// Why a preferences file was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerError {
    /// The file holds more than [`MAX_FILE_BYTES`] bytes: this many.
    TooLarge(usize),
    /// The injected byte bound was exceeded.
    ByteLimit {
        /// Observed UTF-8 bytes.
        bytes: usize,
        /// Allowed bytes.
        limit: u64,
    },
    /// Container depth exceeded the injected limit.
    DepthLimit(usize),
    /// The file is not TOML, a key given twice included: the parser's reason.
    NotToml(String),
    /// The file names no schema.
    NoSchema,
    /// A key the registry does not know, with its tables' names.
    UnknownKey(String),
    /// A locked setting the file sets.
    Locked(String),
    /// A setting the file sets twice, in forms TOML tells apart, such as
    /// `"search.k" = 3` and `k = 9` under `[search]`.
    Twice(String),
    /// A value its setting refuses.
    Refused {
        /// The dotted key.
        key: String,
        /// What the setting expects.
        reason: String,
        /// What the file holds there.
        found: String,
    },
}

impl fmt::Display for LayerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(bytes) => write!(
                formatter,
                "the file holds {bytes} bytes, more than the {MAX_FILE_BYTES} a preferences \
                 file may"
            ),
            Self::ByteLimit { bytes, limit } => write!(
                formatter,
                "the file holds {bytes} bytes, more than the {limit} allowed"
            ),
            Self::DepthLimit(limit) => write!(formatter, "the file is deeper than {limit} levels"),
            Self::NotToml(reason) => write!(formatter, "the file is not TOML: {reason}"),
            Self::NoSchema => write!(formatter, "the file has no schema = {SCHEMA:?} line"),
            Self::UnknownKey(key) => write!(formatter, "unknown key {key:?}"),
            Self::Locked(key) => write!(formatter, "{key}: {LOCKED}"),
            Self::Twice(key) => write!(formatter, "{key}: the setting is set twice in the file"),
            Self::Refused { key, reason, found } => {
                write!(formatter, "{key}: {reason}, found {found}")
            }
        }
    }
}

impl error::Error for LayerError {}
