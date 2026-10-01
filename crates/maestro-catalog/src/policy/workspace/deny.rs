//! Immutable checked deny data; only the composition root supplies platform bindings.
use maestro_filesystem::Directory;
use maestro_kernel::paths::Environment;
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs, io,
    path::{Component, Path, PathBuf},
};

/// Strict built-in schema, with no workspace override or caller-supplied rule loader.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rules {
    /// Required supported generation; deserialization itself validates the value.
    #[serde(rename = "schema")]
    _schema: Schema,
    /// Locations anchored to platform bindings.
    paths: Vec<Location>,
    /// Exact basenames or a single trailing wildcard.
    names: Vec<String>,
}

/// Only this compiled generation is supported; runtime/workspace rules are never loaded.
#[derive(Deserialize)]
enum Schema {
    /// First immutable checked schema.
    #[serde(rename = "maestro-secret-paths/1")]
    V1,
}

/// A path below a named platform binding.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Location {
    /// Binding, never an environment variable supplied by catalog content.
    base: Base,
    /// Normal path components only.
    path: String,
}

/// Platform bindings the built-in data may use.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Base {
    /// Explicit resolved home, including Windows USERPROFILE.
    Home,
    /// XDG configuration or the home fallback.
    Config,
    /// XDG data or the home fallback.
    Data,
    /// Windows APPDATA when supplied.
    Roaming,
    /// Windows LOCALAPPDATA when supplied.
    Local,
}

impl Rules {
    /// Parse strictly and validate the deliberately small pattern language.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let rules: Self = serde_json::from_str(text).map_err(io::Error::other)?;
        for location in &rules.paths {
            if location.path.is_empty()
                || location.path.contains(['*', '\\', ':'])
                || !Path::new(&location.path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
            {
                return Err(io::Error::other(
                    "deny location must contain normal names only",
                ));
            }
        }
        for name in &rules.names {
            let literal = name.strip_suffix('*').unwrap_or(name);
            if literal.is_empty()
                || literal.contains(['*', '/', '\\', ':'])
                || literal == "."
                || literal == ".."
            {
                return Err(io::Error::other("deny basename has an invalid pattern"));
            }
        }
        Ok(rules)
    }
}

/// Canonical anchored locations and basename rules compiled from immutable bytes.
#[derive(Debug)]
pub(super) struct SecretPaths {
    /// Canonical or not-yet-created locations.
    locations: Vec<PathBuf>,
    /// Patterns from checked data, not hard-coded names in Rust.
    names: Vec<String>,
}

impl SecretPaths {
    /// Load the only production rule source and resolve all supplied absolute bindings.
    pub(super) fn built_in(home: &Path, environment: &Environment) -> io::Result<Self> {
        let rules = Rules::parse(include_str!("../../../resources/secret-paths.json"))?;
        let config = absolute(environment.xdg_config_home.as_deref())
            .unwrap_or_else(|| home.join(".config"));
        let data = absolute(environment.xdg_data_home.as_deref())
            .unwrap_or_else(|| home.join(".local/share"));
        let roaming = absolute(environment.app_data.as_deref());
        let local = absolute(environment.local_app_data.as_deref());
        let mut locations = Vec::new();
        for location in rules.paths {
            let base = match location.base {
                Base::Home => Some(home.to_path_buf()),
                Base::Config => Some(config.clone()),
                Base::Data => Some(data.clone()),
                Base::Roaming => roaming.clone(),
                Base::Local => local.clone(),
            };
            if let Some(base) = base {
                // Retain both spellings: a secret alias never makes its original name safe.
                let named = canonical_location(&base)?.join(&location.path);
                locations.push(canonical_location(&named)?);
                locations.push(named);
            }
        }
        Ok(Self {
            locations,
            names: rules.names,
        })
    }

    /// Deny a location or a basename at any depth, including directory ancestors.
    pub(super) fn refuses(&self, path: &Path) -> bool {
        self.locations
            .iter()
            .any(|location| contains(location, path))
            || path.components().any(|part| {
                let Component::Normal(name) = part else {
                    return false;
                };
                self.refuses_name(name)
            })
    }
    /// Basename rules are ASCII-case-insensitive on all hosts, failing closed for non-UTF-8.
    fn refuses_name(&self, name: &OsStr) -> bool {
        let Some(name) = name.to_str() else {
            return true;
        };
        self.names.iter().any(|pattern| {
            if let Some(prefix) = pattern.strip_suffix('*') {
                name.to_ascii_lowercase()
                    .starts_with(&prefix.to_ascii_lowercase())
            } else {
                name.eq_ignore_ascii_case(pattern)
            }
        })
    }
}

/// Component ancestry, conservatively case-insensitive for Windows and default macOS volumes.
/// This is never a string prefix comparison of paths.
pub(super) fn contains(root: &Path, path: &Path) -> bool {
    let mut parts = path.components();
    root.components().all(|expected| {
        parts.next().is_some_and(|actual| {
            if let (Some(expected), Some(actual)) =
                (expected.as_os_str().to_str(), actual.as_os_str().to_str())
            {
                expected.eq_ignore_ascii_case(actual)
            } else {
                expected == actual
            }
        })
    })
}

/// Ignore relative XDG/platform bindings just as the kernel path mapping does.
fn absolute(value: Option<&OsStr>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

/// Resolve a not-yet-created location through its existing ancestor, refusing other failures.
pub(super) fn canonical_location(path: &Path) -> io::Result<PathBuf> {
    canonical_location_result(path, path.canonicalize())
}

/// Test-only failed canonicalization, even if the path subsequently appears absent.
#[cfg(test)]
pub(super) fn canonical_location_failure(path: &Path) -> io::Result<PathBuf> {
    canonical_location_result(
        path,
        Err(io::Error::other("injected canonicalization failure")),
    )
}

/// Keep non-NotFound failures closed and distinguish absence from a dangling entry.
fn canonical_location_result(path: &Path, resolved: io::Result<PathBuf>) -> io::Result<PathBuf> {
    match resolved {
        Ok(canonical) => match Directory::open_canonical(&canonical) {
            Ok(held) => held.canonical_path(),
            Err(error) if error.kind() == io::ErrorKind::NotADirectory => {
                let parent = canonical
                    .parent()
                    .ok_or_else(|| io::Error::other("location has no parent"))?;
                let name = canonical
                    .file_name()
                    .ok_or_else(|| io::Error::other("location has no name"))?;
                Ok(Directory::open_canonical(parent)?
                    .canonical_path()?
                    .join(name))
            }
            Err(error) => Err(error),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Ok(_) => return Err(io::Error::other("deny binding exists but cannot resolve")),
                Err(error) => return Err(error),
            }
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::other("location has no ancestor"))?;
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::other("location has no name"))?;
            Ok(canonical_location(parent)?.join(name))
        }
        Err(error) => Err(error),
    }
}
