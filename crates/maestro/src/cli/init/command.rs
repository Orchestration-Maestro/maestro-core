//! `maestro init`: show the complete authoring plan and apply only on request.
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
    settings::{PreferencesDraft, WorkspacePreferences, draft_preferences, resolve},
    source::{Known, builtin, frozen_rows},
};
use maestro_kernel::workspace::WorkspaceAuthority;
use serde::Serialize;
use std::{
    env,
    io::{self, BufRead, IsTerminal as _, Write},
    path::{Path, PathBuf},
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

/// Confirmed explicit draft choices over the already validated session port.
#[derive(Clone, Copy)]
pub(in crate::cli) struct PreferenceChoices<'a> {
    /// Storage-independent, immutable preference snapshot.
    pub(in crate::cli) source: &'a dyn WorkspacePreferences,
    /// Only explicitly supplied command-line choices.
    pub(in crate::cli) choices: &'a [String],
}

/// Explicit effect choices; neither preview nor generic output flags authorize trust.
#[derive(Clone, Copy)]
pub(in crate::cli) struct ApplyChoices<'a> {
    /// Whether the user requested effects.
    pub(in crate::cli) apply: bool,
    /// Decline trust and separately approve only preferences.
    pub(in crate::cli) preferences_only: bool,
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
        let already_applied = self.preview.plan.is_applied()
            && self
                .preferences
                .as_ref()
                .is_none_or(|draft| draft.files.is_applied());
        let document = InitDocument {
            schema: "maestro-cli/init/1",
            mode: "authoring convenience; not a verified install",
            applied,
            already_applied,
            prerequisites: &self.preview.prerequisites,
            bindings: &self.preview.bindings,
            root: &self.root,
            files: &self.preview.plan,
            preferences: self.preferences.as_ref(),
        };
        if output.is_json() {
            output.result(&document, "")
        } else {
            let text = serde_json::to_string_pretty(&document)
                .map_err(|error| Failure::failed_by(&error))?;
            output.text(&text)
        }
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
        output.text(if self.preview.plan.is_applied() {
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
    catalog_dir: &Path,
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
        return preferences_only(
            output,
            &root,
            &effects,
            PreferenceChoices {
                source: &source,
                choices,
            },
        );
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
    let flags = maestro_settings::parse_flags(&registry, choices)
        .map_err(|error| Failure::refused_by(&error))?;
    let resolved = resolve(
        &registry,
        &maestro_settings::resolve(&registry, &layers, &flags),
    );
    output.with_language(resolved.text("language").unwrap_or("auto"))
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

/// Separate decline path never loads a preset or applies a template/projection plan.
pub(in crate::cli::init) fn preferences_only(
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
pub(in crate::cli::init) fn preferences_only_with_io(
    output: Output,
    root: &Path,
    effects: &ApplyChoices<'_>,
    preferences: PreferenceChoices<'_>,
    (terminal, input, error): (bool, &mut dyn BufRead, &mut dyn Write),
) -> Result<ExitCode, Failure> {
    let draft = prepare_preferences(root, preferences.source, preferences.choices)?;
    apply_preferences_with_io(output, root, effects, &draft, (terminal, input, error))
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
    draft: &PreferencesDraft,
) -> Result<ExitCode, Failure> {
    let stdin = io::stdin();
    let stderr = io::stderr();
    apply_preferences_with_io(
        output,
        root,
        effects,
        draft,
        (
            stdin.is_terminal() && stderr.is_terminal(),
            &mut stdin.lock(),
            &mut stderr.lock(),
        ),
    )
}

/// Existing trusted confirmation and journal writer, shared by both decline paths.
fn apply_preferences_with_io(
    output: Output,
    root: &Path,
    effects: &ApplyChoices<'_>,
    draft: &PreferencesDraft,
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
