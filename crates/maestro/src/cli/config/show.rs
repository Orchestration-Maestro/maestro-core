//! `config get`, `config list` and `config explain`: a session's effective
//! settings, each with the layer that set it.

use super::super::output::Output;
use crate::{failure::Failure, settings::Session};
use maestro_settings::{ResolvedSetting, SettingDescriptor, Source, Value};
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
    let resolved = session.resolved();
    let setting = resolved.get(key).ok_or_else(|| unknown(key))?;
    let document = json!({
        "schema": "maestro-cli/config-get/1",
        "key": key,
        "value": setting.value.to_json(),
        "source": source_json(&setting.source),
    });
    output.result(&document, &setting.value.to_string())?;
    Ok(ExitCode::SUCCESS)
}

/// Prints every setting with its effective value and the layer that set it.
///
/// # Errors
///
/// [`Failure::Failed`] when stdout cannot be written to.
pub(in crate::cli) fn list(output: Output, session: &Session) -> Result<ExitCode, Failure> {
    let resolved = session.resolved();
    let mut text = String::new();
    let mut settings = Vec::new();
    for setting in resolved.iter() {
        // Writing to a String cannot fail.
        let _written = writeln!(
            text,
            "{} = {}  ({})",
            setting.descriptor.key,
            setting.value.to_toml(),
            layer_name(&setting.source)
        );
        settings.push(json!({
            "key": setting.descriptor.key,
            "value": setting.value.to_json(),
            "source": source_json(&setting.source),
        }));
    }
    let document = json!({"schema": "maestro-cli/config-list/1", "settings": settings});
    output.result(&document, text.trim_end())?;
    Ok(ExitCode::SUCCESS)
}

/// Explains `key`, or every setting without one, after the files the
/// session read.
///
/// # Errors
///
/// [`Failure::Refused`] for a key no setting has.
pub(in crate::cli) fn explain(
    output: Output,
    session: &Session,
    key: Option<&str>,
) -> Result<ExitCode, Failure> {
    let resolved = session.resolved();
    let selected: Vec<&ResolvedSetting<'_>> = match key {
        Some(key) => vec![resolved.get(key).ok_or_else(|| unknown(key))?],
        None => resolved.iter().collect(),
    };
    let mut text = files_text(session);
    let mut settings = Vec::with_capacity(selected.len());
    for setting in selected {
        text.push('\n');
        text.push_str(&setting_text(setting));
        settings.push(setting_json(setting));
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

/// The short name of the layer of `source`.
fn layer_name(source: &Source) -> &'static str {
    match source {
        Source::Default => "default",
        Source::File { layer, .. } => layer.name(),
        Source::Flag => "--set",
    }
}

/// `source` as JSON: its layer, and a file's path.
fn source_json(source: &Source) -> Json {
    match source {
        Source::File { layer, path } => json!({"layer": layer.name(), "path": path}),
        Source::Default | Source::Flag => json!({"layer": layer_name(source)}),
    }
}

/// The explanation of one setting, for people.
fn setting_text(setting: &ResolvedSetting<'_>) -> String {
    let descriptor = setting.descriptor;
    let mut text = format!(
        "{} = {}\n  set by: {}\n",
        descriptor.key,
        setting.value.to_toml(),
        setting.source
    );
    for (source, value) in &setting.overridden {
        // Writing to a String cannot fail.
        let _written = writeln!(text, "  overrides: {source} ({})", value.to_toml());
    }
    let _written = writeln!(
        text,
        "  accepts: {}; default {}; class {}\n  {}",
        descriptor.kind.expectation(),
        descriptor.default,
        descriptor.class.name(),
        descriptor.description
    );
    text
}

/// The explanation of one setting, as JSON.
fn setting_json(setting: &ResolvedSetting<'_>) -> Json {
    let descriptor: &SettingDescriptor = setting.descriptor;
    let overridden: Vec<Json> = setting
        .overridden
        .iter()
        .map(|(source, value)| json!({"source": source_json(source), "value": value.to_json()}))
        .collect();
    json!({
        "key": descriptor.key,
        "value": setting.value.to_json(),
        "source": source_json(&setting.source),
        "overridden": overridden,
        "kind": descriptor.kind,
        "default": descriptor.default,
        "class": descriptor.class,
        "description": descriptor.description,
    })
}

/// The files the session read, for people.
fn files_text(session: &Session) -> String {
    let mut text = format!("user file: {}\n", presence(&session.files.user));
    // Writing to a String cannot fail.
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

/// `path` and whether it exists.
fn presence(path: &Path) -> String {
    if path.is_file() {
        path.display().to_string()
    } else {
        format!("{} (absent: defaults apply)", path.display())
    }
}

/// The value of a setting as `config set` takes it, for a message.
pub(super) fn shown(value: Option<&Value>) -> String {
    value.map_or_else(|| "unset".to_owned(), Value::to_toml)
}
