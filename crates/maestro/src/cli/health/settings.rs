//! The settings check: which preferences files a session reads, the user's
//! and the project's, and the refusal of one that is not valid, naming its
//! key. The kernel's `config.toml` has a check of its own.

use super::check::Check;
use crate::{failure::Failure, settings::Session};
use maestro_settings::USER_FILE;
use std::path::Path;

/// The check of the session `session`, whose user file is in `config_dir`.
pub(super) fn settings_check(config_dir: &Path, session: Result<&Session, Failure>) -> Check {
    let target = config_dir.join(USER_FILE).display().to_string();
    let session = match session {
        Ok(session) => session,
        Err(error) => {
            return Check::failed(
                "settings",
                &target,
                error.to_string(),
                "fix the named preferences file, then run maestro doctor again",
            );
        }
    };
    let user = if session.layers.user.is_some() {
        "user file read"
    } else {
        "user file absent (defaults apply)"
    };
    let project = match (&session.files.project, &session.discovery.note) {
        (Some(path), _) => format!("project file {} read", path.display()),
        (None, Some(note)) => format!("no project file: {note}"),
        (None, None) => "no project file found".to_owned(),
    };
    let mut detail = format!("{user}; {project}");
    for skipped in &session.discovery.skipped {
        detail.push_str("; ");
        detail.push_str(skipped);
    }
    Check::passed("settings", &target, detail)
}
