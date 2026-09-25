//! Where this machine keeps the artifacts the tokenizer profile fingerprints.
use super::process::read_bounded;
use crate::Error;
use serde::Deserialize;
use std::{
    env,
    ffi::OsString,
    fs::File,
    path::{Path, PathBuf},
};

/// The environment variable that names the binding file.
const BINDING_VARIABLE: &str = "MAESTRO_NATIVE_BINDING";
/// The only binding schema this version reads.
const BINDING_SCHEMA: &str = "maestro-native-binding/1";
/// A binding is a few paths; anything larger is not one.
const MAX_BINDING_BYTES: usize = 1024 * 1024;

/// Local paths of the qualified artifacts, resolved from a binding file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeBinding {
    /// The GGUF model the counter loads.
    pub(crate) model: PathBuf,
    /// The vocabulary-only counter executable.
    pub(crate) counter: PathBuf,
    /// The directory holding the counter's shared libraries.
    pub(crate) library_directory: PathBuf,
    /// The llama.cpp source tree the counter was built from.
    pub(crate) source_root: PathBuf,
}

/// The binding file as written, before its paths are resolved.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingFile {
    /// Must equal [`BINDING_SCHEMA`].
    schema: String,
    /// See [`NativeBinding::model`].
    model: PathBuf,
    /// See [`NativeBinding::counter`].
    counter: PathBuf,
    /// See [`NativeBinding::library_directory`].
    library_directory: PathBuf,
    /// See [`NativeBinding::source_root`].
    source_root: PathBuf,
}

/// Every binding refusal names the variable and the schema, never a path or
/// the file's contents.
fn refusal(problem: &str) -> Error {
    Error(format!(
        "{problem}: {BINDING_VARIABLE} must name a {BINDING_SCHEMA} file (see TOKENIZER.md)"
    ))
}

impl NativeBinding {
    /// Read the binding file that `MAESTRO_NATIVE_BINDING` names.
    pub(crate) fn from_environment() -> Result<Self, Error> {
        Self::from_variable(env::var_os(BINDING_VARIABLE))
    }

    /// Read the binding file a variable's value names; an unset variable is refused.
    pub(crate) fn from_variable(value: Option<OsString>) -> Result<Self, Error> {
        let path = value.ok_or_else(|| refusal("no native binding"))?;
        Self::from_file(Path::new(&path))
    }

    /// Read a binding file; a relative path resolves against its directory.
    pub(crate) fn from_file(path: &Path) -> Result<Self, Error> {
        let file = File::open(path).map_err(|_| refusal("unreadable native binding"))?;
        let bytes = read_bounded(file, MAX_BINDING_BYTES)
            .map_err(|_| refusal("native binding unreadable or over 1 MiB"))?;
        let written: BindingFile =
            serde_json::from_slice(&bytes).map_err(|_| refusal("invalid native binding"))?;
        if written.schema != BINDING_SCHEMA {
            return Err(refusal("unsupported native binding schema"));
        }
        let base = path.parent().unwrap_or_else(|| Path::new(""));
        let resolve = |bound: PathBuf| {
            if bound.is_absolute() {
                bound
            } else {
                base.join(bound)
            }
        };
        Ok(Self {
            model: resolve(written.model),
            counter: resolve(written.counter),
            library_directory: resolve(written.library_directory),
            source_root: resolve(written.source_root),
        })
    }
}
