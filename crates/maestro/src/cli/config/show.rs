//! `config get`, `config list` and `config explain`: effective values,
//! declared classes, restrictive layers and any ignored widening requests.

use super::super::output::Output;
use crate::{failure::Failure, settings::Session};
use maestro_catalog::settings::{ResolvedSettings, ResolvedValue};
use maestro_settings::{SettingDescriptor, Value};
use serde_json::{Value as Json, json};
use std::{fmt::Write as _, path::Path, process::ExitCode};

/// Prints the effective value of `key`.
///
/// # Errors
///
/// [`Failure::Refused`] for a key no setting has.
pub(in crate::cli) fn get(
    output: Output,
    session: &Session,
    key: &str,
) -> Result<ExitCode, Failure> {
    let descriptor = session.registry.get(key).ok_or_else(|| unknown(key))?;
    let result = session.catalog_resolved();
    let setting = result
        .get(key)
        .ok_or_else(|| Failure::failed("registry descriptor has no resolution"))?;
    let setting = setting
        .as_ref()
        .map_err(|error| Failure::refused(&error.message))?;
    let diagnostics = diagnostics_for(&result, key);
    let document = json!({
        "schema": "maestro-cli/config-get/1",
        "key": key,
        "value": setting.value().to_json(),
        "source": source_json(session, setting.source()),
        "class": descriptor.class.name(),
        "diagnostics": diagnostics,
    });
    output.result(&document, &setting.value().to_string())?;
    Ok(ExitCode::SUCCESS)
}

/// Prints every setting with its effective value, class, source and diagnostics.
///
/// # Errors
///
/// [`Failure::Failed`] when stdout cannot be written to.
pub(in crate::cli) fn list(output: Output, session: &Session) -> Result<ExitCode, Failure> {
    let resolved = session.catalog_resolved();
    let mut text = String::new();
    let mut settings = Vec::new();
    for descriptor in session.registry.descriptors() {
        let setting = resolved
            .get(&descriptor.key)
            .ok_or_else(|| Failure::failed("registry descriptor has no resolution"))?
            .as_ref()
            .map_err(|error| Failure::refused(&error.message))?;
        let diagnostics = diagnostics_for(&resolved, &descriptor.key);
        let _written = writeln!(
            text,
            "{} = {}  ({}; class {}){}",
            descriptor.key,
            setting.value().to_toml(),
            setting.source(),
            descriptor.class.name(),
            diagnostic_suffix(&diagnostics),
        );
        settings.push(setting_json(session, descriptor, setting, &diagnostics));
    }
    let document = json!({"schema": "maestro-cli/config-list/1", "settings": settings});
    output.result(&document, text.trim_end())?;
    Ok(ExitCode::SUCCESS)
}

/// Explains `key`, or every setting, after the files the session read.
///
/// # Errors
///
/// [`Failure::Refused`] for a key no setting has.
pub(in crate::cli) fn explain(
    output: Output,
    session: &Session,
    key: Option<&str>,
) -> Result<ExitCode, Failure> {
    let resolved = session.catalog_resolved();
    let selected: Vec<&SettingDescriptor> = match key {
        Some(key) => vec![session.registry.get(key).ok_or_else(|| unknown(key))?],
        None => session.registry.descriptors().collect(),
    };
    let mut text = files_text(session);
    let mut settings = Vec::with_capacity(selected.len());
    for descriptor in selected {
        let setting = resolved
            .get(&descriptor.key)
            .ok_or_else(|| Failure::failed("registry descriptor has no resolution"))?
            .as_ref()
            .map_err(|error| Failure::refused(&error.message))?;
        let diagnostics = diagnostics_for(&resolved, &descriptor.key);
        text.push('\n');
        text.push_str(&setting_text(descriptor, setting, &diagnostics));
        settings.push(setting_json(session, descriptor, setting, &diagnostics));
    }
    let document = json!({
        "schema": "maestro-cli/config-explain/1",
        "files": files_json(session),
        "settings": settings,
    });
    output.result(&document, text.trim_end())?;
    Ok(ExitCode::SUCCESS)
}

/// The refusal of `key`, which no setting has.
pub(super) fn unknown(key: &str) -> Failure {
    Failure::refused(format!(
        "unknown key {key:?}: `maestro config list` names every setting"
    ))
}

/// Diagnostics for one effective setting.
fn diagnostics_for(resolved: &ResolvedSettings, key: &str) -> Vec<String> {
    resolved
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.key == key)
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

/// Layer provenance with the file path when the value came from a file.
fn source_json(session: &Session, source: &str) -> Json {
    match source {
        "user" => json!({"layer": "user", "path": session.files.user}),
        "workspace" => json!({"layer": "project", "path": session.files.project}),
        "flag" => json!({"layer": "--set"}),
        layer => json!({"layer": layer}),
    }
}

/// A short human-readable suffix for ignored requests.
fn diagnostic_suffix(diagnostics: &[String]) -> String {
    if diagnostics.is_empty() {
        String::new()
    } else {
        format!("; diagnostic: {}", diagnostics.join(", "))
    }
}

/// The explanation of one setting, for people.
fn setting_text(
    descriptor: &SettingDescriptor,
    setting: &ResolvedValue,
    diagnostics: &[String],
) -> String {
    let mut text = format!(
        "{} = {}\n  set by: {}\n  class: {}\n",
        descriptor.key,
        setting.value().to_toml(),
        setting.source(),
        descriptor.class.name(),
    );
    for (source, value) in setting.overridden() {
        let _written = writeln!(
            text,
            "  overridden: {} ({})",
            source.name(),
            value.to_toml()
        );
    }
    for diagnostic in diagnostics {
        let _written = writeln!(text, "  diagnostic: {diagnostic}");
    }
    let _written = writeln!(
        text,
        "  accepts: {}; default {}; {}",
        descriptor.kind.expectation(),
        descriptor.default,
        descriptor.description
    );
    text
}

/// The explanation of one setting, as JSON.
fn setting_json(
    session: &Session,
    descriptor: &SettingDescriptor,
    setting: &ResolvedValue,
    diagnostics: &[String],
) -> Json {
    let overridden: Vec<Json> = setting
        .overridden()
        .iter()
        .map(|(source, value)| {
            json!({
                "source": source_json(session, source.name()),
                "value": value.to_json(),
            })
        })
        .collect();
    json!({
        "key": descriptor.key,
        "value": setting.value().to_json(),
        "source": source_json(session, setting.source()),
        "overridden": overridden,
        "kind": descriptor.kind,
        "default": descriptor.default,
        "class": descriptor.class.name(),
        "description": descriptor.description,
        "diagnostics": diagnostics,
    })
}

/// The files the session read, for people.
fn files_text(session: &Session) -> String {
    let mut text = format!("user file: {}\n", presence(&session.files.user));
    let _written = if let Some(path) = &session.files.project {
        writeln!(text, "project file: {}", presence(path))
    } else {
        let reason = session
            .discovery
            .note
            .as_deref()
            .unwrap_or("none found upward from the working directory");
        writeln!(text, "project file: none ({reason})")
    };
    for skipped in &session.discovery.skipped {
        let _written = writeln!(text, "  {skipped}");
    }
    text
}

/// The files the session read, as JSON.
fn files_json(session: &Session) -> Json {
    json!({
        "user": {
            "path": session.files.user,
            "exists": session.files.user.is_file(),
        },
        "project": {
            "path": session.files.project,
            "note": session.discovery.note,
            "skipped": session.discovery.skipped,
        },
    })
}

/// The value of a setting as `config set` takes it, for a message.
pub(super) fn shown(value: Option<&Value>) -> String {
    value.map_or_else(|| "unset".to_owned(), Value::to_toml)
}

/// `path` and whether it exists.
fn presence(path: &Path) -> String {
    if path.is_file() {
        path.display().to_string()
    } else {
        format!("{} (absent: defaults apply)", path.display())
    }
}
