//! `maestro init`: show the complete authoring plan and apply only on request.
use crate::{
    cli::{output::Output, trust, trust_path},
    failure::Failure,
};
use maestro_catalog::{
    bootstrap::{self, DirectoryPresets, Prerequisite},
    files::{self, FilePlan},
    limits::Limits,
    policy::workspace::{CheckedTrust, JournalTrust, WorkspaceTrust as _, write_preferences},
    settings::{PreferencesDraft, WorkspacePreferences, draft_preferences},
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
    /// Separate root-local preference draft; trust is checked before CLI apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    preferences: Option<&'a PreferencesDraft>,
}

/// Confirmed explicit draft choices over the already validated session port.
#[derive(Clone, Copy)]
pub(super) struct PreferenceChoices<'a> {
    /// Storage-independent, immutable preference snapshot.
    pub(super) source: &'a dyn WorkspacePreferences,
    /// Only explicitly supplied command-line choices.
    pub(super) choices: &'a [String],
}

/// Explicit effect choices; neither preview nor generic output flags authorize trust.
#[derive(Clone, Copy)]
pub(super) struct ApplyChoices<'a> {
    /// Whether the user requested effects.
    pub(super) apply: bool,
    /// Decline trust and separately approve only preferences.
    pub(super) preferences_only: bool,
    /// Exact canonical path for the separate preferences-only approval.
    pub(super) confirm_path: Option<&'a Path>,
}

/// Preview a composed project and, when requested, apply it through C04.
pub(super) fn run(
    output: Output,
    catalog_dir: &Path,
    presets: &[String],
    effects: ApplyChoices<'_>,
    preference_choices: PreferenceChoices<'_>,
) -> Result<ExitCode, Failure> {
    let PreferenceChoices { source, choices } = preference_choices;
    let should_apply = effects.apply;
    let root = env::current_dir()
        .and_then(|root| root.canonicalize())
        .map_err(|error| Failure::failed_by(&error))?;
    if should_apply && !effects.preferences_only {
        require_trust(&root)?;
    }
    if effects.preferences_only {
        return preferences_only(output, &root, &effects, preference_choices);
    }
    let preferences = if choices.is_empty() {
        None
    } else {
        Some(
            draft_preferences(&root, source, choices, &Limits::PRODUCTION)
                .map_err(Failure::refused)?,
        )
    };
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
        preferences: preferences.as_ref(),
    };
    let text =
        serde_json::to_string_pretty(&document).map_err(|error| Failure::failed_by(&error))?;
    output.text(&text)?;
    if should_apply {
        bootstrap::apply(&root, &preview).map_err(|error| Failure::refused(error.to_string()))?;
        if let Some(preferences) = &preferences {
            files::apply(&root, &preferences.files).map_err(|error| Failure::refused_by(&error))?;
        }
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

/// CLI decision point only; C05j owns the later held-handle enforcement boundary.
fn require_trust(root: &Path) -> Result<(), Failure> {
    let boundaries = trust::boundaries()?;
    boundaries.check_root(root).map_err(Failure::refused)?;
    let database = trust::database()?;
    let adapter = JournalTrust::new(&database);
    if CheckedTrust::new(&adapter, &boundaries)
        .containing_root(root)
        .is_none()
    {
        return Err(Failure::refused(format!(
            "workspace is untrusted; {}",
            trust_path::suggestion(root)
        )));
    }
    Ok(())
}

/// Separate decline path never loads a preset or applies a template/projection plan.
fn preferences_only(
    output: Output,
    root: &Path,
    effects: &ApplyChoices<'_>,
    preferences: PreferenceChoices<'_>,
) -> Result<ExitCode, Failure> {
    trust::boundaries()?
        .check_root(root)
        .map_err(Failure::refused)?;
    let Some(confirmation) = trust::approve(root, effects.confirm_path).map_err(|failure| {
        let instruction = if let Some(quoted) = trust_path::quoted_canonical(root) {
            format!("repeat init with --preferences-only --confirm-path {quoted}")
        } else {
            format!(
                "quote the canonical path for your shell; path (data):\n{:?}",
                trust_path::visible_path(root)
            )
        };
        Failure::refused(format!(
            "preferences-only write requires separate confirmation: {failure}; {instruction}"
        ))
    })?
    else {
        return Err(Failure::refused(
            "preferences-only write declined; no files written",
        ));
    };
    let draft = draft_preferences(
        root,
        preferences.source,
        preferences.choices,
        &Limits::PRODUCTION,
    )
    .map_err(Failure::refused)?;
    write_preferences(&trust::database()?, root, &draft.file, confirmation)
        .map_err(Failure::refused)?;
    output.result(
        &draft,
        "Wrote preferences only; no workspace trust granted.",
    )?;
    Ok(ExitCode::SUCCESS)
}
