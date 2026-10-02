//! Exact-path or default-no terminal approval; preferences-only writes stay separate.
use super::port::{CheckedTrust, JournalTrust};
use crate::file_input::FileInput;
use maestro_kernel::{
    artifact::Digest,
    store::Database,
    workspace::{Answer, Confirmation, WorkspaceAnswer},
};
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
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

/// Non-cloneable, single-use confirmation for exactly one root-local preferences write.
/// Its private fields cannot be supplied by a model or catalog. Only the trusted IO
/// confirmation adapter constructs it; consumption never records workspace approval.
///
/// ```compile_fail
/// use maestro_catalog::{files::FileInput, policy::workspace::{
///     CheckedTrust, PreferencesConfirmation, write_preferences,
/// }};
/// use maestro_kernel::store::Database;
/// fn reuse(database: &Database, trust: &CheckedTrust<'_>, file: &FileInput,
///     permit: PreferencesConfirmation) {
///     write_preferences(database, trust, file, permit).unwrap();
///     write_preferences(database, trust, file, permit).unwrap(); // already consumed
/// }
/// ```
#[derive(Debug)]
pub struct PreferencesConfirmation {
    /// Exact canonical root repeated or confirmed at the user-only IO boundary.
    root: PathBuf,
    /// Kernel receipt provenance, not a caller-selected bypass boolean.
    confirmation: Confirmation,
}

/// Confirm only the root-local config through the trusted user-only IO adapter.
///
/// # Errors
/// Refuses unresolved paths and missing or mismatched user confirmation.
pub fn preferences_confirmation(
    canonical: &Path,
    confirm_path: Option<&Path>,
    terminal: bool,
    prompt: &str,
    io: (&mut dyn BufRead, &mut dyn Write),
) -> Result<Option<PreferencesConfirmation>, String> {
    let resolved = canonical
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let accepted = confirmation(canonical, confirm_path, terminal, prompt, io)?;
    Ok(accepted.map(|confirmation| PreferencesConfirmation {
        root: resolved,
        confirmation,
    }))
}

/// Write only the separately approved preferences file with kernel-local digest receipts.
/// No C04 workspace state, template, projection or broad HOME grant is created.
///
/// # Errors
/// Refuses any other target, existing bytes, links and failed durable writes or journal records.
pub fn write_preferences(
    database: &Database,
    trust: &CheckedTrust<'_>,
    file: &FileInput,
    permit: PreferencesConfirmation,
) -> Result<(), String> {
    let PreferencesConfirmation { root, confirmation } = permit;
    let root = root.as_path();
    let adapter = JournalTrust::optional(None);
    let narrow = CheckedTrust::preferences(&adapter, trust.boundaries, root)?;
    narrow
        .check_write(root, Path::new(&file.path))
        .map_err(|error| error.to_string())?;
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
    narrow
        .authorize_create(root, Path::new(&file.path))
        .and_then(|path| path.write_new(&file.bytes))
        .map_err(|error| error.to_string())?;
    record(Answer::Preferences {
        digest,
        confirmation,
        completed: true,
    })?;
    Ok(())
}
