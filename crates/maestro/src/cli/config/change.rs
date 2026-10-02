//! `config set` and `config unset`: one setting written in one preferences
//! file, in place, and journaled. The value is checked first; the file is
//! then read and changed under its lock, never through a link; the edit is
//! verified against the whole file, the kernel is opened before the file
//! changes, and a change the journal cannot record is undone, or the
//! recovery copy named when the undo cannot be confirmed: every change that
//! lands is recorded. Writing the same value again changes and records
//! nothing. The kernel's `config.toml` is never written.

use super::{
    super::output::{Output, diagnose},
    show::shown,
};
use crate::{cli::init::flow::validate, failure::Failure, kernel::Kernel};
use maestro_kernel::{
    paths::{self, Environment},
    scope::LOCAL,
    settings::SettingChange,
};
use maestro_settings::{
    FileEdit, FileError, FilePlace, Layer, LayerName, MAX_FILE_BYTES, MAX_FILE_DEPTH,
    PROJECT_DIRECTORY, PROJECT_FILE, Registry, USER_FILE, Value, discover_project_file,
    set_in_document, unset_in_document,
};
use serde_json::json;
use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Where `config set` and `config unset` find their files.
#[derive(Debug, Clone)]
pub(in crate::cli) struct Places {
    /// The configuration directory, which holds the user file.
    pub(in crate::cli) config_dir: PathBuf,
    /// The working directory, where project discovery starts.
    pub(in crate::cli) working: Option<PathBuf>,
    /// The home directory, which bounds project discovery.
    pub(in crate::cli) home: Option<PathBuf>,
}

impl Places {
    /// The places of this process: the configuration directory the
    /// environment names, the working directory and the home directory.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when no configuration directory can be found.
    pub(in crate::cli) fn current() -> Result<Self, Failure> {
        Ok(Self {
            config_dir: paths::config_dir(&Environment::current())
                .map_err(|error| Failure::failed_by(&error))?,
            working: env::current_dir().ok(),
            home: env::home_dir(),
        })
    }
}

/// One change: `key` set to the text `value`, or unset without one, in the
/// file of `layer`.
#[derive(Debug, Clone, Copy)]
pub(in crate::cli) struct Change<'a> {
    /// The setting's key.
    pub(in crate::cli) key: &'a str,
    /// Its new value as the command line writes it; `None` unsets it.
    pub(in crate::cli) value: Option<&'a str>,
    /// The file to write.
    pub(in crate::cli) layer: LayerName,
}

/// Applies `change` in the file `places` give, then journals it in the
/// kernel `open_kernel` opens.
///
/// # Errors
///
/// As [`run_with`].
pub(in crate::cli) fn run(
    output: Output,
    change: Change<'_>,
    places: &Places,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<ExitCode, Failure> {
    run_with(output, change, places, open_kernel, |kernel, record| {
        kernel
            .database
            .record_setting_change(record)
            .map(drop)
            .map_err(|error| error.to_string())
    })
}

/// Applies `change` in the file `places` give, then records it with
/// `journal`, which says why it could not, in the kernel `open_kernel`
/// opens.
///
/// # Errors
///
/// [`Failure::Refused`] for an unknown or locked key, a refused value, a
/// project file outside home, a link, a file that cannot be edited safely,
/// or one another program changed meanwhile; and [`Failure::Failed`] when
/// the file cannot be written or the change cannot be journaled, the file
/// then put back as it was, or its recovery copy named.
pub(in crate::cli) fn run_with(
    output: Output,
    change: Change<'_>,
    places: &Places,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
    journal: impl FnOnce(&Kernel, &SettingChange) -> Result<(), String>,
) -> Result<ExitCode, Failure> {
    let registry = Registry::built_in().map_err(|error| Failure::failed_by(&error))?;
    run_in_registry(output, change, (places, &registry), open_kernel, journal)
}

/// The editor uses the same settings API with its admitted descriptor snapshot.
pub(in crate::cli) fn run_in_registry(
    output: Output,
    change: Change<'_>,
    (places, registry): (&Places, &Registry),
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
    journal: impl FnOnce(&Kernel, &SettingChange) -> Result<(), String>,
) -> Result<ExitCode, Failure> {
    let new = validate(registry, change.key, change.value, change.layer)?;
    let path = target(change.layer, places)?;
    let place = match change.layer {
        LayerName::User => FilePlace::user(&places.config_dir),
        LayerName::Project => FilePlace::project(&path)
            .ok_or_else(|| Failure::failed(format!("{} has no directory", path.display())))?,
    };
    let mut file = FileEdit::begin(place, new.is_some(), || {
        diagnose(&format!(
            "waiting for another maestro command changing {}",
            path.display()
        ));
    })
    .map_err(|error| file_failure(&error))?;
    let edited = edit(registry, &path, file.before(), change.key, new.as_ref())?;
    let document = |changed: bool, old: Option<&Value>| {
        json!({
            "schema": "maestro-cli/config-change/1",
            "key": change.key,
            "old": old.map(Value::to_json),
            "new": new.as_ref().map(Value::to_json),
            "layer": change.layer.name(),
            "file": path,
            "changed": changed,
        })
    };
    let Some((text, old)) = edited else {
        let old = new.as_ref();
        let line = format!(
            "{} is {} in {} already",
            change.key,
            shown(old),
            path.display()
        );
        output.json_result(&document(false, old), &line)?;
        return Ok(ExitCode::SUCCESS);
    };
    let kernel = open_kernel()?;
    file.publish(&text).map_err(|error| file_failure(&error))?;
    let record = SettingChange {
        principal: LOCAL.to_owned(),
        key: change.key.to_owned(),
        old: old.as_ref().map(Value::to_json),
        new: new.as_ref().map(Value::to_json),
        layer: change.layer.name().to_owned(),
        file: path.display().to_string(),
    };
    if let Err(error) = journal(&kernel, &record) {
        return Err(match file.undo() {
            Ok(()) => Failure::failed(format!(
                "the change of {} could not be journaled, so {} is left as it was: {error}",
                change.key,
                path.display()
            )),
            Err(restore) => Failure::failed(format!(
                "the change of {} could not be journaled ({error}), and {restore}",
                change.key
            )),
        });
    }
    file.keep();
    let line = format!(
        "{} = {} in {} (was {})",
        change.key,
        shown(new.as_ref()),
        path.display(),
        shown(old.as_ref())
    );
    output.json_result(&document(true, old.as_ref()), &line)?;
    Ok(ExitCode::SUCCESS)
}

/// The failure of a file edit: a refusal, or a failure to write.
fn file_failure(error: &FileError) -> Failure {
    if error.is_refusal() {
        Failure::refused(error)
    } else {
        Failure::failed(error)
    }
}

/// The file of `layer`: the user file, or the nearest project file upward
/// from the working directory within home, else a new one there; the edit
/// refuses either when it is, or lies in, a link.
///
/// # Errors
///
/// [`Failure::Refused`] for a project file where none would be read.
pub(in crate::cli) fn target(layer: LayerName, places: &Places) -> Result<PathBuf, Failure> {
    if layer == LayerName::User {
        return Ok(places.config_dir.join(USER_FILE));
    }
    let working = places
        .working
        .as_deref()
        .ok_or_else(|| Failure::refused("--project: the working directory is unknown"))?;
    let discovery = discover_project_file(working, places.home.as_deref());
    if let Some(note) = discovery.note {
        return Err(Failure::refused(format!("--project: {note}")));
    }
    if let Some(file) = discovery.file {
        return Ok(file);
    }
    let working = working
        .canonicalize()
        .map_err(|error| Failure::refused(format!("--project: {error}")))?;
    // A `.maestro` here that is a link, which discovery skipped, is refused
    // when the edit opens it.
    Ok(working.join(PROJECT_DIRECTORY).join(PROJECT_FILE))
}

/// The text of the file `path`, `before`, with `key` set to `new` or unset,
/// and the value it held; `None` when nothing changes.
fn edit(
    registry: &Registry,
    path: &Path,
    before: Option<&str>,
    key: &str,
    new: Option<&Value>,
) -> Result<Option<(String, Option<Value>)>, Failure> {
    let refuse = |error: &dyn Error| Failure::refused(format!("{}: {error}", path.display()));
    let old = match before {
        Some(text) => {
            Layer::parse_preferences(registry, text, MAX_FILE_BYTES as u64, MAX_FILE_DEPTH)
                .map_err(|error| refuse(&error))?
                .get(key)
                .cloned()
        }
        None => None,
    };
    if old.as_ref() == new {
        return Ok(None);
    }
    // A key the file sets is in a file that exists.
    let text = match new {
        Some(value) => set_in_document(registry, before, key, value),
        None => unset_in_document(registry, before.unwrap_or_default(), key)
            .map(Option::unwrap_or_default),
    }
    .map_err(|error| refuse(&error))?;
    Ok(Some((text, old)))
}
