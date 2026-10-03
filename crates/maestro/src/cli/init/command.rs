//! `maestro init`: show the complete authoring plan and apply only on request.
use super::{flow::Draft, plain::terminal};
use crate::{
    cli::{catalog, output::Output, session, trust, trust_path},
    failure::Failure,
    presentation::messages::MessageKey,
};
use maestro_catalog::{
    bootstrap::{self, AreaInventories, Prerequisite},
    files::{self, FilePlan},
    limits::Limits,
    policy::workspace::{
        Access, CheckedTrust, JournalTrust, WorkspaceTrust as _, write_preferences,
    },
    settings::{PreferencesDraft, WorkspacePreferences, draft_preferences},
    source::{Known, builtin, frozen_rows},
};
use maestro_kernel::workspace::WorkspaceAuthority;
use maestro_settings::LayerName;
use serde::Serialize;
use std::{
    env,
    io::{self, BufRead, IsTerminal as _, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    str::from_utf8,
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
    /// Inventory-required binding references, reported without resolving or executing.
    bindings: &'a [String],
    /// The project root shown to the user.
    /// Canonical workspace selected by init.
    root: &'a Path,
    /// The C04 digest-bound file plan.
    files: &'a FilePlan,
    /// Separate root-local preference draft; trust is checked before CLI apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Independent C04 preference file plan.
    preferences: Option<&'a PreferencesDraft>,
}

/// Explicit effect choices; neither preview nor generic output flags authorize trust.
#[derive(Clone, Copy)]
pub(in crate::cli) struct ApplyChoices<'a> {
    /// Whether the user requested effects.
    pub(in crate::cli) apply: bool,
    /// Decline trust and separately approve only preferences.
    pub(in crate::cli) preferences_only: bool,
    /// Never prompt when scripted choices were explicitly requested.
    pub(in crate::cli) non_interactive: bool,
    /// Exact canonical path for the separate preferences-only approval.
    pub(in crate::cli) confirm_path: Option<&'a Path>,
}

/// One pinned planner result; review and apply consume these exact file plans.
pub(in crate::cli::init) struct Prepared {
    /// Canonical workspace selected by init.
    root: PathBuf,
    /// The existing C05 bootstrap plan and captured source fingerprints.
    preview: bootstrap::BootstrapPreview,
    /// Independent C04 preferences file plan.
    preferences: Option<PreferencesDraft>,
    /// Admitted S1 defaults used by the settings editor.
    pub(in crate::cli::init) registry: maestro_settings::Registry,
}

/// The existing C05 planner, reused by scripts and interactive review.
pub(in crate::cli::init) fn prepare(
    catalog_dir: &Path,
    presets: &[String],
    source: &dyn WorkspacePreferences,
    choices: &[String],
) -> Result<Prepared, Failure> {
    let root = env::current_dir()
        .and_then(|root| root.canonicalize())
        .map_err(|error| Failure::failed_by(&error))?;
    let boundaries = trust::boundaries()?;
    let database = trust::existing_database()?;
    let adapter = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let checked = CheckedTrust::new(&adapter, &boundaries);
    let registry = builtin().map_err(Failure::failed)?;
    let settings = source.registry().map_err(Failure::refused)?;
    let rows = frozen_rows();
    let provider = AreaInventories::new(
        catalog_dir,
        registry,
        Known {
            rows: &rows,
            settings: &settings,
            today: catalog::today()?,
        },
    )
    .map_err(Failure::refused)?
    .with_compiled_backends(session::compiled_backends());
    let admitted = InitPreferences {
        source,
        registry: provider.admitted_registry(),
    };
    let preferences = preference_draft(&root, &admitted, choices, &checked)?;
    let preview =
        bootstrap::preview(&root, &provider, presets, &checked).map_err(Failure::refused)?;
    Ok(Prepared {
        root,
        preview,
        preferences,
        registry: provider.admitted_registry().clone(),
    })
}

impl Prepared {
    /// The existing versioned document, identical in plain and scripted paths.
    pub(in crate::cli::init) fn show(&self, output: Output, applied: bool) -> Result<(), Failure> {
        if output.is_json() {
            output.result(&self.document(applied), "")
        } else {
            output.text(&self.text(applied)?)
        }
    }

    /// One shared human formatter for scripts, plain and terminal review.
    pub(in crate::cli::init) fn text(&self, applied: bool) -> Result<String, Failure> {
        serde_json::to_string_pretty(&self.document(applied))
            .map_err(|error| Failure::failed_by(&error))
    }

    /// Preserve the existing document and key order in every renderer.
    fn document(&self, applied: bool) -> InitDocument<'_> {
        InitDocument {
            schema: "maestro-cli/init/1",
            mode: "authoring convenience; not a verified install",
            applied,
            already_applied: self.already_applied(),
            prerequisites: &self.preview.prerequisites,
            bindings: &self.preview.bindings,
            root: &self.root,
            files: &self.preview.plan,
            preferences: self.preferences.as_ref(),
        }
    }

    /// Both independent plans must be unchanged before reporting a no-op.
    fn already_applied(&self) -> bool {
        self.preview.plan.is_applied()
            && self
                .preferences
                .as_ref()
                .is_none_or(|draft| draft.files.is_applied())
    }

    /// Human review uses the pinned config bytes, not the machine hex encoding.
    pub(in crate::cli::init) fn review_values(&self) -> Result<String, Failure> {
        review_values(&self.root, self.preferences.as_ref())
    }

    /// Recheck real authority after confirmation; never replan the reviewed bytes.
    pub(in crate::cli::init) fn apply(&self, output: Output) -> Result<(), Failure> {
        require_trust(output, &self.root)?;
        let boundaries = trust::boundaries()?;
        let database = trust::existing_database()?;
        let adapter = JournalTrust::optional(
            database
                .as_ref()
                .map(|database| database as &dyn WorkspaceAuthority),
        );
        let checked = CheckedTrust::new(&adapter, &boundaries);
        bootstrap::apply(&self.root, &self.preview, &checked).map_err(Failure::refused)?;
        if let Some(preferences) = &self.preferences {
            files::apply(&self.root, &preferences.files, &checked).map_err(Failure::refused)?;
        }
        output.text(if self.already_applied() {
            "Already applied; no files written."
        } else {
            "Applied authoring plan."
        })?;
        output.text(&format!(
            "Config: {}. Inspect with `maestro config explain`.",
            self.root.join(".maestro/config.toml").display()
        ))
    }
}

/// Script dispatch retains C47a's independent preference snapshot.
pub(in crate::cli::init) fn scripted(
    output: Output,
    catalog_dir: Option<&Path>,
    presets: &[String],
    effects: ApplyChoices<'_>,
    choices: &[String],
) -> Result<ExitCode, Failure> {
    let source = session::init_preferences()?;
    let output = preference_output(output, &source, choices)?;
    if effects.preferences_only {
        let root = env::current_dir()
            .and_then(|root| root.canonicalize())
            .map_err(|error| Failure::failed_by(&error))?;
        let draft = prepare_preferences(&root, &source, choices)?;
        return apply_preferences(output, &root, &effects, (&draft, choices));
    }
    let catalog_dir = catalog_dir.ok_or_else(|| {
        Failure::refused(
            "scripted init requires --catalog-dir and --preset; use --plain for labelled prompts",
        )
    })?;
    if presets.is_empty() {
        return Err(Failure::refused(
            "scripted init requires an explicit --preset",
        ));
    }
    let prepared = prepare(catalog_dir, presets, &source, choices)?;
    if !output.is_json() || !effects.apply {
        prepared.show(output, false)?;
    }
    if effects.apply {
        prepared.apply(output)?;
        if output.is_json() {
            prepared.show(output, true)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Init localizes from its own preferences without a runtime session or admitted lock.
pub(in crate::cli::init) fn preference_output(
    output: Output,
    source: &dyn WorkspacePreferences,
    choices: &[String],
) -> Result<Output, Failure> {
    let registry = source.registry().map_err(Failure::refused)?;
    let layers = source
        .layers(&registry, &Limits::PRODUCTION)
        .map_err(Failure::refused)?;
    Draft::new(registry, layers, LayerName::Project, choices, true)?.language_output(output)
}

/// Existing config without explicit choices is checked but never adopted or rewritten.
fn preference_draft(
    root: &Path,
    source: &dyn WorkspacePreferences,
    choices: &[String],
    checked: &CheckedTrust<'_>,
) -> Result<Option<PreferencesDraft>, Failure> {
    // Any other read failure falls through: the preferences port already refuses an
    // unreadable, foreign or linked config before init gets here.
    let existing = choices.is_empty()
        && checked
            .authorize(root, Path::new(".maestro/config.toml"), Access::Read)
            .and_then(|path| path.open_read())
            .is_ok();
    if existing {
        return Ok(None);
    }
    draft_preferences(root, source, choices, &Limits::PRODUCTION, checked)
        .map(Some)
        .map_err(Failure::refused)
}

/// CLI decision point only; C05j owns the later held-handle enforcement boundary.
pub(in crate::cli::init) fn require_trust(output: Output, root: &Path) -> Result<(), Failure> {
    let boundaries = trust::boundaries()?;
    boundaries.check_root(root).map_err(Failure::refused)?;
    let database = trust::existing_database()?;
    let adapter = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
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

/// Narrow decline review uses the existing draft planner and no catalog inventory.
pub(in crate::cli::init) fn prepare_preferences(
    root: &Path,
    source: &dyn WorkspacePreferences,
    choices: &[String],
) -> Result<PreferencesDraft, Failure> {
    let boundaries = trust::boundaries()?;
    boundaries.check_root(root).map_err(Failure::refused)?;
    let database = trust::existing_database()?;
    let adapter = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let checked = CheckedTrust::new(&adapter, &boundaries);
    draft_preferences(root, source, choices, &Limits::PRODUCTION, &checked)
        .map_err(Failure::refused)
}

/// Confirm and persist the exact preferences bytes shown by the narrow review.
pub(in crate::cli::init) fn apply_preferences(
    output: Output,
    root: &Path,
    effects: &ApplyChoices<'_>,
    reviewed: (&PreferencesDraft, &[String]),
) -> Result<ExitCode, Failure> {
    let stdin = io::stdin();
    let stderr = io::stderr();
    apply_preferences_with_io(
        output,
        root,
        effects,
        reviewed,
        (
            terminal(stdin.is_terminal(), stderr.is_terminal()),
            &mut stdin.lock(),
            &mut stderr.lock(),
        ),
    )
}

/// Existing trusted confirmation and journal writer, shared by both decline paths.
pub(in crate::cli::init) fn apply_preferences_with_io(
    output: Output,
    root: &Path,
    effects: &ApplyChoices<'_>,
    (draft, choices): (&PreferencesDraft, &[String]),
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
        terminal && !effects.non_interactive && !output.is_json(),
        (input, error),
    ) {
        Ok(confirmation) => confirmation,
        Err(failure) => {
            let instruction = preference_retry(output, root, choices)?;
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
    write_preferences(&trust::database()?, &checked, &draft.file, confirmation)
        .map_err(Failure::refused)?;
    output.result(
        draft,
        &output.wording(MessageKey::InitPreferencesWritten, &[])?,
    )?;
    Ok(ExitCode::SUCCESS)
}

/// Init's existing preference snapshot with the newly checked lowest defaults slot.
struct InitPreferences<'a> {
    /// User/workspace layers remain the already selected immutable session source.
    source: &'a dyn WorkspacePreferences,
    /// Checked manifest-backed defaults, not resolved preference winners.
    registry: &'a maestro_settings::Registry,
}

impl WorkspacePreferences for InitPreferences<'_> {
    fn registry(&self) -> Result<maestro_settings::Registry, String> {
        Ok(self.registry.clone())
    }

    fn layers(
        &self,
        registry: &maestro_settings::Registry,
        limits: &Limits,
    ) -> Result<maestro_settings::Layers, String> {
        self.source.layers(registry, limits)
    }
}

/// Show the exact pinned preferences as text without altering the machine document.
pub(in crate::cli::init) fn review_values(
    root: &Path,
    preferences: Option<&PreferencesDraft>,
) -> Result<String, Failure> {
    let values = match preferences {
        Some(draft) => from_utf8(&draft.file.bytes).map_err(|error| Failure::failed_by(&error))?,
        None => "Existing config preserved unchanged.\n",
    };
    Ok(format!(
        "Root: {}\nConfig values (.maestro/config.toml):\n{values}",
        root.display()
    ))
}

/// A declined interactive flow can retry the reviewed choices without any catalog flags.
fn preference_retry(output: Output, root: &Path, choices: &[String]) -> Result<String, Failure> {
    let quoted: Option<Vec<_>> = choices
        .iter()
        .map(|choice| trust_path::quoted_argument(choice))
        .collect();
    if let (Some(path), Some(choices)) = (trust_path::quoted_canonical(root), quoted) {
        let mut command = format!("maestro init --apply --preferences-only --confirm-path {path}");
        for choice in choices {
            command.push_str(" --set ");
            command.push_str(&choice);
        }
        Ok(format!(
            "{}: `{command}`",
            output.wording(MessageKey::InitConfirmCommand, &[("path", &path)])?
        ))
    } else {
        let path = format!("{:?}", trust_path::visible_path(root));
        Ok(format!(
            "{}\nRetry: maestro init --apply --preferences-only --confirm-path PATH; \
            pass each reviewed assignment with --set, quoting it for your shell \
            (data): {choices:?}",
            output.wording(MessageKey::InitConfirmData, &[("path", &path)])?
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{InitPreferences, preference_output};
    use crate::{cli::output::Output, presentation::messages::MessageKey};
    use maestro_catalog::settings::FilePreferences;
    use maestro_settings::Registry;
    use maestro_test_scratch::scratch_directory;
    use std::fs;

    #[test]
    fn catalog_init_command_admitted_defaults_retain_preference_layers() {
        let root = scratch_directory().unwrap();
        fs::write(
            root.join("preferences.toml"),
            "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n",
        )
        .unwrap();
        let registry = Registry::built_in().unwrap();
        let source = FilePreferences::new(&root, &root);
        let admitted = InitPreferences {
            source: &source,
            registry: &registry,
        };
        let output = preference_output(Output::new(false), &admitted, &[]).unwrap();
        let actual = output.wording(MessageKey::FlowTone, &[]).unwrap();
        let expected = Output::new(false)
            .with_language("fr")
            .unwrap()
            .wording(MessageKey::FlowTone, &[])
            .unwrap();
        assert_eq!(actual, expected);
        assert_ne!(
            actual,
            Output::new(false)
                .wording(MessageKey::FlowTone, &[])
                .unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
