//! Exact-path or default-no terminal approval; preferences-only writes stay separate.
use crate::files::FileInput;
use maestro_filesystem::Directory;
use maestro_kernel::{
    artifact::Digest,
    store::Database,
    workspace::{Answer, Confirmation, WorkspaceAnswer},
};
use std::{
    io::{BufRead, Write},
    path::Path,
};

/// Obtain a real user confirmation for the canonical path, defaulting to no.
/// The caller supplies rendered prompt data and terminal status from its trusted IO adapter,
/// never from tool text.
///
/// # Errors
/// Missing non-terminal confirmation and mismatched canonical spellings refuse.
pub fn confirmation(
    canonical: &Path,
    confirm_path: Option<&Path>,
    terminal: bool,
    prompt: &str,
    (input, output): (&mut dyn BufRead, &mut dyn Write),
) -> Result<Option<Confirmation>, String> {
    if let Some(confirm_path) = confirm_path {
        if confirm_path.as_os_str() != canonical.as_os_str() {
            return Err(format!(
                "confirmation must repeat the exact canonical path {}",
                canonical.display()
            ));
        }
        return Ok(Some(Confirmation::ConfirmPath));
    }
    if !terminal {
        return Err("user confirmation required (terminal or --confirm-path)".to_owned());
    }
    write!(output, "{prompt}").map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())?;
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    if answer.trim().eq_ignore_ascii_case("yes") || answer.trim().eq_ignore_ascii_case("y") {
        Ok(Some(Confirmation::Terminal))
    } else {
        Ok(None)
    }
}

/// Write only the separately approved preferences file with kernel-local digest receipts.
/// No C04 workspace state, template, projection or broad HOME grant is created.
///
/// # Errors
/// Refuses any other target, existing bytes, links and failed durable writes or journal records.
pub fn write_preferences(
    database: &Database,
    root: &Path,
    file: &FileInput,
    confirmation: Confirmation,
) -> Result<(), String> {
    if file.path != ".maestro/config.toml" {
        return Err("preferences-only approval cannot write another file".to_owned());
    }
    let digest = format!("sha256:{}", Digest::of(&file.bytes).as_str());
    let record = |answer| {
        database
            .record_workspace_answer(&WorkspaceAnswer {
                path: root.to_path_buf(),
                answer,
            })
            .map_err(|error| error.to_string())
    };
    record(Answer::Declined)?;
    record(Answer::Preferences {
        digest: digest.clone(),
        confirmation,
        completed: false,
    })?;
    let directory =
        Directory::open(root, Path::new(".maestro"), true).map_err(|error| error.to_string())?;
    let mut output = directory
        .create_new("config.toml")
        .map_err(|error| error.to_string())?;
    output
        .write_all(&file.bytes)
        .and_then(|()| output.sync_all())
        .and_then(|()| directory.sync())
        .map_err(|error| error.to_string())?;
    record(Answer::Preferences {
        digest,
        confirmation,
        completed: true,
    })?;
    Ok(())
}
