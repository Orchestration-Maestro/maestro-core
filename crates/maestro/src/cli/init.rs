//! `maestro init`: show the complete authoring plan and apply only on request.
use crate::{
    cli::{output::Output, trust, trust_path},
    failure::Failure,
    presentation::messages::MessageKey,
};
use maestro_catalog::{
    bootstrap::{self, DirectoryPresets, Prerequisite},
    files::{self, FilePlan},
    limits::Limits,
    policy::workspace::{
        Access, CheckedTrust, JournalTrust, WorkspaceTrust as _, write_preferences,
    },
    settings::{PreferencesDraft, WorkspacePreferences, draft_preferences},
};
use maestro_kernel::workspace::WorkspaceAuthority;
use serde::Serialize;
use std::{
    env,
    io::{self, BufRead, IsTerminal as _, Write},
    path::Path,
    process::ExitCode,
};

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
        require_trust(output, &root)?;
    }
    if effects.preferences_only {
        return preferences_only(output, &root, &effects, preference_choices);
    }
    let boundaries = trust::boundaries()?;
    let database = trust::existing_database()?;
    let adapter = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let checked = CheckedTrust::new(&adapter, &boundaries);
    let preferences = preference_draft(&root, source, choices, &checked)?;
    let provider = DirectoryPresets::new(catalog_dir);
    let preview =
        bootstrap::preview(&root, &provider, presets, &checked).map_err(Failure::refused)?;
    let already_applied = preview.plan.is_applied()
        && preferences
            .as_ref()
            .is_none_or(|draft| draft.files.is_applied());
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
        bootstrap::apply(&root, &preview, &checked)
            .map_err(|error| Failure::refused(error.to_string()))?;
        if let Some(preferences) = &preferences {
            files::apply(&root, &preferences.files, &checked)
                .map_err(|error| Failure::refused_by(&error))?;
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

/// Existing config without explicit choices is checked but never adopted or rewritten.
fn preference_draft(
    root: &Path,
    source: &dyn WorkspacePreferences,
    choices: &[String],
    checked: &CheckedTrust<'_>,
) -> Result<Option<PreferencesDraft>, Failure> {
    if choices.is_empty() {
        match checked
            .authorize(root, Path::new(".maestro/config.toml"), Access::Read)
            .and_then(|path| path.open_read())
        {
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Failure::refused_by(&error)),
        }
    }
    draft_preferences(root, source, choices, &Limits::PRODUCTION, checked)
        .map(Some)
        .map_err(Failure::refused)
}

/// CLI decision point only; C05j owns the later held-handle enforcement boundary.
fn require_trust(output: Output, root: &Path) -> Result<(), Failure> {
    let boundaries = trust::boundaries()?;
    boundaries.check_root(root).map_err(Failure::refused)?;
    let database = trust::database()?;
    let adapter = JournalTrust::new(&database);
    if CheckedTrust::new(&adapter, &boundaries)
        .containing_root(root)
        .is_none()
    {
        let instruction = if let Some(path) = trust_path::quoted_canonical(root) {
            output.wording(MessageKey::InitTrustCommand, &[("path", &path)])?
        } else {
            let path = format!("{:?}", trust_path::visible_path(root));
            output.wording(MessageKey::InitTrustData, &[("path", &path)])?
        };
        return Err(Failure::refused(output.wording(
            MessageKey::InitUntrusted,
            &[("instruction", &instruction)],
        )?));
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
    let stdin = io::stdin();
    let stderr = io::stderr();
    preferences_only_with_io(
        output,
        root,
        effects,
        preferences,
        (
            stdin.is_terminal() && stderr.is_terminal(),
            &mut stdin.lock(),
            &mut stderr.lock(),
        ),
    )
}

/// Preferences-only execution shares rendered prompt hand-off with injected IO.
fn preferences_only_with_io(
    output: Output,
    root: &Path,
    effects: &ApplyChoices<'_>,
    preferences: PreferenceChoices<'_>,
    (terminal, input, error): (bool, &mut dyn BufRead, &mut dyn Write),
) -> Result<ExitCode, Failure> {
    trust::boundaries()?
        .check_root(root)
        .map_err(Failure::refused)?;
    let prompt = output.wording(
        MessageKey::InitApprovePrompt,
        &[("path", &trust_path::visible_path(root))],
    )?;
    let confirmation = match trust::approve_preferences_with_io(
        root,
        effects.confirm_path,
        &prompt,
        terminal,
        (input, error),
    ) {
        Ok(confirmation) => confirmation,
        Err(failure) => {
            let instruction = if let Some(path) = trust_path::quoted_canonical(root) {
                output.wording(MessageKey::InitConfirmCommand, &[("path", &path)])?
            } else {
                let path = format!("{:?}", trust_path::visible_path(root));
                output.wording(MessageKey::InitConfirmData, &[("path", &path)])?
            };
            return Err(Failure::refused(output.wording(
                MessageKey::InitConfirm,
                &[
                    ("failure", &failure.to_string()),
                    ("instruction", &instruction),
                ],
            )?));
        }
    };
    let Some(confirmation) = confirmation else {
        return Err(Failure::refused(
            output.wording(MessageKey::InitPreferencesDeclined, &[])?,
        ));
    };
    let boundaries = trust::boundaries()?;
    let database = trust::existing_database()?;
    let adapter = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let checked = CheckedTrust::new(&adapter, &boundaries);
    let draft = draft_preferences(
        root,
        preferences.source,
        preferences.choices,
        &Limits::PRODUCTION,
        &checked,
    )
    .map_err(Failure::refused)?;
    write_preferences(&trust::database()?, &checked, &draft.file, confirmation)
        .map_err(Failure::refused)?;
    output.result(
        &draft,
        &output.wording(MessageKey::InitPreferencesWritten, &[])?,
    )?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::{ApplyChoices, PreferenceChoices, preferences_only_with_io};
    use crate::cli::{output::Output, trust_path};
    use maestro_catalog::settings::FilePreferences;
    use maestro_test_scratch::scratch_directory;
    use std::fs;

    #[test]
    fn catalog_presentation_init_terminal_handoff_is_localized_and_blank_declines() {
        let root = scratch_directory().unwrap().canonicalize().unwrap();
        let source = FilePreferences::new(&root, &root);
        let visible = trust_path::visible_path(&root);
        for (language, expected, declined) in [
            (
                "fr",
                format!("Approuver {visible} ? [y/N] "),
                "écriture des préférences seules refusée ; aucun fichier écrit",
            ),
            (
                "es",
                format!("¿Aprobar {visible}? [y/N] "),
                "se rechazó escribir solo las preferencias; no se escribió ningún archivo",
            ),
        ] {
            let output = Output::new(false).with_language(language).unwrap();
            let mut rendered = Vec::new();
            let result = preferences_only_with_io(
                output,
                &root,
                &ApplyChoices {
                    apply: true,
                    preferences_only: true,
                    confirm_path: None,
                },
                PreferenceChoices {
                    source: &source,
                    choices: &[],
                },
                (true, &mut "\n".as_bytes(), &mut rendered),
            );
            assert_eq!(String::from_utf8(rendered).unwrap(), expected);
            assert_eq!(result.unwrap_err().to_string(), declined);
            assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        }
    }
}
