//! `maestro init`: show the complete authoring plan and apply only on request.
use crate::{cli::output::Output, failure::Failure};
use maestro_catalog::{
    bootstrap::{self, DirectoryPresets, Prerequisite},
    files::FilePlan,
};
use serde::Serialize;
use std::{env, path::Path, process::ExitCode};

/// Machine-readable preview, or preview plus the requested owned-file apply.
#[derive(Serialize)]
struct InitDocument<'a> {
    /// Output schema version.
    schema: &'static str,
    /// States that this is authoring, not verified installation.
    mode: &'static str,
    /// Whether an explicit apply completed successfully.
    applied: bool,
    /// Whether preview found this exact plan already committed and unchanged.
    already_applied: bool,
    /// Each manifest-declared prerequisite, checked without invocation.
    prerequisites: &'a [Prerequisite],
    /// The project root shown to the user.
    root: &'a Path,
    /// The C04 digest-bound file plan.
    files: &'a FilePlan,
}

/// Preview a composed project and, when requested, apply it through C04.
pub(super) fn run(
    output: Output,
    catalog_dir: &Path,
    presets: &[String],
    should_apply: bool,
) -> Result<ExitCode, Failure> {
    let root = env::current_dir().map_err(|error| Failure::failed_by(&error))?;
    let provider = DirectoryPresets::new(catalog_dir);
    let preview = bootstrap::preview(&root, &provider, presets).map_err(Failure::refused)?;
    let already_applied = preview.plan.is_applied();
    let mut document = InitDocument {
        schema: "maestro-cli/init/1",
        mode: "authoring convenience; not a verified install",
        applied: false,
        already_applied,
        prerequisites: &preview.prerequisites,
        root: &root,
        files: &preview.plan,
    };
    let text =
        serde_json::to_string_pretty(&document).map_err(|error| Failure::failed_by(&error))?;
    output.text(&text)?;
    if should_apply {
        bootstrap::apply(&root, &preview).map_err(|error| Failure::refused(error.to_string()))?;
        document.applied = true;
        output.text(if already_applied {
            "Already applied; no files written."
        } else {
            "Applied authoring plan."
        })?;
    }
    if output.is_json() {
        output.result(&document, "")?;
    }
    Ok(ExitCode::SUCCESS)
}
