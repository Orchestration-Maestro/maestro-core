//! `config.toml`, the kernel's configuration file in its configuration
//! directory ([`config_dir`](crate::paths::config_dir)), and the grants it
//! gives the local principal. In S1 it holds what the local user may read:
//!
//! ```toml
//! [access]
//! read = ['workspace/default/collection/ctm']
//! ```
//!
//! It follows `bindings.toml`'s pattern ([`crate::binding`]): the file is
//! checked whole when it is read, each refusal is typed and names it, a key
//! it does not name is refused, and a missing file grants nothing.
//! [`Database::apply_config`](crate::store::Database::apply_config) then
//! reconciles the local principal's grants with it.

use super::path::{InvalidScope, Scope};
use std::{
    collections::BTreeSet,
    error, fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

/// The configuration file's name in the kernel's configuration directory.
pub const CONFIG_FILE: &str = "config.toml";

/// The principal of the local user, which the CLI and the MCP server run as:
/// [`CONFIG_FILE`] says what it may read.
pub const LOCAL: &str = "local";

/// What [`CONFIG_FILE`] holds: in S1, the scopes the local principal may
/// read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// The scopes of `access.read`.
    pub(super) read: BTreeSet<Scope>,
}

impl Config {
    /// The configuration of [`CONFIG_FILE`] in `config_dir`; one that grants
    /// nothing when the file does not exist.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Io`] when the file exists but cannot be read, and the
    /// refusals of [`Config::from_str`] for its text.
    pub fn load(config_dir: &Path) -> Result<Self, ConfigError> {
        let path = config_dir.join(CONFIG_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => text.parse(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Io { path, source }),
        }
    }
}

impl FromStr for Config {
    type Err = ConfigError;

    /// The configuration a `config.toml` holds.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NotToml`] when the text is not TOML, a key given twice
    /// included; [`ConfigError::UnknownKey`] for a key the file does not
    /// name; [`ConfigError::Shape`] for a value of another shape than its
    /// key's; and [`ConfigError::InvalidScope`] for a scope the file grants
    /// that is not a scope path.
    fn from_str(text: &str) -> Result<Self, ConfigError> {
        let table = text
            .parse::<toml::Table>()
            .map_err(|error| ConfigError::NotToml(error.to_string()))?;
        let mut config = Self::default();
        for (key, value) in table {
            match (key.as_str(), value) {
                ("access", toml::Value::Table(access)) => config.read = readable(access)?,
                ("access", _) => {
                    return Err(ConfigError::Shape {
                        key,
                        expected: "a table",
                    });
                }
                _ => return Err(ConfigError::UnknownKey(key)),
            }
        }
        Ok(config)
    }
}

/// The scopes the `[access]` table `access` lets the local principal read.
fn readable(access: toml::Table) -> Result<BTreeSet<Scope>, ConfigError> {
    let mut read = BTreeSet::new();
    for (key, value) in access {
        if key != "read" {
            return Err(ConfigError::UnknownKey(format!("access.{key}")));
        }
        read = scopes(value)?;
    }
    Ok(read)
}

/// The scopes of `access.read`, an array of scope paths.
fn scopes(value: toml::Value) -> Result<BTreeSet<Scope>, ConfigError> {
    let shape = || ConfigError::Shape {
        key: "access.read".to_owned(),
        expected: "an array of scope paths",
    };
    let toml::Value::Array(paths) = value else {
        return Err(shape());
    };
    paths
        .into_iter()
        .map(|path| match path {
            toml::Value::String(path) => path.parse().map_err(ConfigError::InvalidScope),
            _ => Err(shape()),
        })
        .collect()
}

/// Why [`CONFIG_FILE`] was refused.
#[derive(Debug)]
pub enum ConfigError {
    /// The file is not TOML, a key given twice included; the parser's
    /// reason.
    NotToml(String),
    /// The file holds a key it does not name, given with its table's name
    /// (`access.write`).
    UnknownKey(String),
    /// A key holds a value of another shape than its own.
    Shape {
        /// The key, with its table's name (`access.read`).
        key: String,
        /// What it must hold.
        expected: &'static str,
    },
    /// A scope the file grants is not a scope path.
    InvalidScope(InvalidScope),
    /// The file exists but cannot be read.
    Io {
        /// The configuration file.
        path: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotToml(reason) => {
                write!(formatter, "{CONFIG_FILE} is not valid TOML: {reason}")
            }
            Self::UnknownKey(key) => write!(
                formatter,
                "{CONFIG_FILE} holds `{key}`, which is not one of its keys"
            ),
            Self::Shape { key, expected } => {
                write!(formatter, "`{key}` in {CONFIG_FILE} must be {expected}")
            }
            Self::InvalidScope(invalid) => {
                write!(
                    formatter,
                    "{CONFIG_FILE} grants what is not a scope: {invalid}"
                )
            }
            Self::Io { path, .. } => write!(formatter, "cannot read {}", path.display()),
        }
    }
}

impl error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::InvalidScope(invalid) => Some(invalid),
            Self::Io { source, .. } => Some(source),
            Self::NotToml(_) | Self::UnknownKey(_) | Self::Shape { .. } => None,
        }
    }
}
