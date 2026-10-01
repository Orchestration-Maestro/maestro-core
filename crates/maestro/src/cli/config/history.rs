//! `config history`: the journaled changes of the local principal's
//! settings, oldest first.

use super::{super::output::Output, show::shown};
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::scope::LOCAL;
use serde_json::{Value as Json, json};
use std::{fmt::Write as _, process::ExitCode};

/// Prints every settings change the kernel `open_kernel` opens journaled for
/// the local principal.
///
/// # Errors
///
/// [`Failure::Failed`] when the kernel cannot be opened or read.
pub(in crate::cli) fn run(
    output: Output,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<ExitCode, Failure> {
    let kernel = open_kernel()?;
    let changes = kernel
        .database
        .setting_changes(LOCAL)
        .map_err(|error| Failure::failed_by(&error))?;
    let mut text = String::new();
    let mut documents = Vec::with_capacity(changes.len());
    for recorded in &changes {
        let change = &recorded.change;
        // Writing to a String cannot fail.
        let _written = writeln!(
            text,
            "{} {} {}: {} -> {} ({} file {})",
            recorded.time,
            change.principal,
            change.key,
            json_shown(change.old.as_ref()),
            json_shown(change.new.as_ref()),
            change.layer,
            change.file
        );
        documents.push(json!({
            "id": recorded.id.to_string(),
            "time": recorded.time,
            "principal": change.principal,
            "key": change.key,
            "old": change.old,
            "new": change.new,
            "layer": change.layer,
            "file": change.file,
        }));
    }
    if changes.is_empty() {
        text.push_str("no settings change is recorded");
    }
    let document = json!({"schema": "maestro-cli/config-history/1", "changes": documents});
    output.result(&document, text.trim_end())?;
    Ok(ExitCode::SUCCESS)
}

/// A journaled value as a message shows it.
fn json_shown(value: Option<&Json>) -> String {
    value.map_or_else(|| shown(None), Json::to_string)
}
