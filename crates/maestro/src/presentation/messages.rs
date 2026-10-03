//! Deterministic interface templates; preferences never affect machine artifacts.
use serde::Deserialize;
use std::{collections::BTreeSet, sync::OnceLock};

/// One embedded file and one registry entry per interface language, English first.
pub(super) const BUILT_INS: &[(&str, &str)] = &[
    ("en", include_str!("languages/en.json")),
    ("fr", include_str!("languages/fr.json")),
    ("es", include_str!("languages/es.json")),
];

/// Typed interface identities, not status codes or command names.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MessageKey {
    /// Localized plain-flow label or navigation prompt.
    FlowWorkspace,
    /// Localized plain-flow label or navigation prompt.
    FlowLanguage,
    /// Localized plain-flow label or navigation prompt.
    FlowTone,
    /// Localized plain-flow label or navigation prompt.
    FlowSettings,
    /// Localized plain-flow label or navigation prompt.
    FlowReview,
    /// Localized plain-flow label or navigation prompt.
    FlowEditorPrompt,
    /// Localized plain-flow label or navigation prompt.
    FlowApplyPrompt,
    /// Localized plain-flow label or navigation prompt.
    FlowPreviewPrompt,
    /// One visible note for conversation languages without interface translations.
    InterfaceFallback,
    /// Init requires separately approved workspace trust.
    InitUntrusted,
    /// Shell-safe explicit trust instruction.
    InitTrustCommand,
    /// Unsafe-to-quote paths remain escaped data, not shell commands.
    InitTrustData,
    /// Preferences-only writes need their own confirmation.
    InitConfirm,
    /// Default-no terminal confirmation for the visible canonical path.
    InitApprovePrompt,
    /// Repeat init with a shell-safe canonical path.
    InitConfirmCommand,
    /// Quote the escaped canonical path with the user's shell.
    InitConfirmData,
    /// Only preferences were written; no authority was granted.
    InitPreferencesWritten,
    /// A declined confirmation writes nothing.
    InitPreferencesDeclined,
    /// CLI-authored catalog missing ownership wording.
    CatalogMissingOwnership,
    /// CLI-authored catalog check passed wording.
    CatalogCheckPassed,
    /// CLI-authored catalog codeowners regular wording.
    CatalogCodeownersRegular,
    /// CLI-authored diagnostic path wording.
    DiagnosticPath,
    /// CLI-authored catalog codeowners missing wording.
    CatalogCodeownersMissing,
    /// CLI-authored catalog codeowners passed wording.
    CatalogCodeownersPassed,
    /// CLI-authored catalog compiled wording.
    CatalogCompiled,
    /// CLI-authored catalog output filename wording.
    CatalogOutputFilename,
    /// CLI-authored catalog staging changed wording.
    CatalogStagingChanged,
    /// CLI-authored catalog output regular wording.
    CatalogOutputRegular,
    /// CLI-authored catalog index passed wording.
    CatalogIndexPassed,
    /// CLI-authored catalog evidence filename wording.
    CatalogEvidenceFilename,
    /// CLI-authored catalog evidence limit wording.
    CatalogEvidenceLimit,
    /// CLI-authored catalog owners passed wording.
    CatalogOwnersPassed,
    /// CLI-authored catalog target untrusted wording.
    CatalogTargetUntrusted,
    /// CLI-authored catalog target approve wording.
    CatalogTargetApprove,
    /// CLI-authored diagnostic instruction wording.
    DiagnosticInstruction,
    /// CLI-authored catalog target declined wording.
    CatalogTargetDeclined,
    /// CLI-authored output write failed wording.
    OutputWriteFailed,
    /// CLI-authored run repair dispatch wording.
    RunRepairDispatch,
    /// CLI-authored run retrieval dispatch wording.
    RunRetrievalDispatch,
    /// CLI-authored run prepare models off wording.
    RunPrepareModelsOff,
    /// CLI-authored run publish models off wording.
    RunPublishModelsOff,
    /// CLI-authored run model dispatch wording.
    RunModelDispatch,
    /// CLI-authored session workspace directory wording.
    SessionWorkspaceDirectory,
    /// CLI-authored session backend unavailable wording.
    SessionBackendUnavailable,
    /// CLI-authored trust home unavailable wording.
    TrustHomeUnavailable,
    /// CLI-authored trust approve wording.
    TrustApprove,
    /// CLI-authored trust recorded wording.
    TrustRecorded,
    /// CLI-authored trust suggestion command wording.
    TrustSuggestionCommand,
    /// CLI-authored trust suggestion data wording.
    TrustSuggestionData,
}

/// Required keys; serde refuses missing, duplicate, mistyped and unknown fields.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Translation {
    /// Plain-flow label or navigation prompt.
    flow_workspace: String,
    /// Plain-flow label or navigation prompt.
    flow_language: String,
    /// Plain-flow label or navigation prompt.
    flow_tone: String,
    /// Plain-flow label or navigation prompt.
    flow_settings: String,
    /// Plain-flow label or navigation prompt.
    flow_review: String,
    /// Plain-flow label or navigation prompt.
    flow_editor_prompt: String,
    /// Plain-flow label or navigation prompt.
    flow_apply_prompt: String,
    /// Plain-flow label or navigation prompt.
    flow_preview_prompt: String,
    /// Conversation language placeholder.
    interface_fallback: String,
    /// Already rendered trust instruction placeholder.
    init_untrusted: String,
    /// Literal shell-safe path placeholder.
    init_trust_command: String,
    /// Escaped path data placeholder.
    init_trust_data: String,
    /// English refusal detail and localized instruction placeholders.
    init_confirm: String,
    /// Visible canonical path in a default-no terminal prompt.
    init_approve_prompt: String,
    /// Literal shell-safe path placeholder.
    init_confirm_command: String,
    /// Escaped path data placeholder.
    init_confirm_data: String,
    /// Preferences-only completion.
    init_preferences_written: String,
    /// Preferences-only cancellation.
    init_preferences_declined: String,
    /// CLI-authored catalog missing ownership wording.
    catalog_missing_ownership: String,
    /// CLI-authored catalog check passed wording.
    catalog_check_passed: String,
    /// CLI-authored catalog codeowners regular wording.
    catalog_codeowners_regular: String,
    /// CLI-authored diagnostic path wording.
    diagnostic_path: String,
    /// CLI-authored catalog codeowners missing wording.
    catalog_codeowners_missing: String,
    /// CLI-authored catalog codeowners passed wording.
    catalog_codeowners_passed: String,
    /// CLI-authored catalog compiled wording.
    catalog_compiled: String,
    /// CLI-authored catalog output filename wording.
    catalog_output_filename: String,
    /// CLI-authored catalog staging changed wording.
    catalog_staging_changed: String,
    /// CLI-authored catalog output regular wording.
    catalog_output_regular: String,
    /// CLI-authored catalog index passed wording.
    catalog_index_passed: String,
    /// CLI-authored catalog evidence filename wording.
    catalog_evidence_filename: String,
    /// CLI-authored catalog evidence limit wording.
    catalog_evidence_limit: String,
    /// CLI-authored catalog owners passed wording.
    catalog_owners_passed: String,
    /// CLI-authored catalog target untrusted wording.
    catalog_target_untrusted: String,
    /// CLI-authored catalog target approve wording.
    catalog_target_approve: String,
    /// CLI-authored diagnostic instruction wording.
    diagnostic_instruction: String,
    /// CLI-authored catalog target declined wording.
    catalog_target_declined: String,
    /// CLI-authored output write failed wording.
    output_write_failed: String,
    /// CLI-authored run repair dispatch wording.
    run_repair_dispatch: String,
    /// CLI-authored run retrieval dispatch wording.
    run_retrieval_dispatch: String,
    /// CLI-authored run prepare models off wording.
    run_prepare_models_off: String,
    /// CLI-authored run publish models off wording.
    run_publish_models_off: String,
    /// CLI-authored run model dispatch wording.
    run_model_dispatch: String,
    /// CLI-authored session workspace directory wording.
    session_workspace_directory: String,
    /// CLI-authored session backend unavailable wording.
    session_backend_unavailable: String,
    /// CLI-authored trust home unavailable wording.
    trust_home_unavailable: String,
    /// CLI-authored trust approve wording.
    trust_approve: String,
    /// CLI-authored trust recorded wording.
    trust_recorded: String,
    /// CLI-authored trust suggestion command wording.
    trust_suggestion_command: String,
    /// CLI-authored trust suggestion data wording.
    trust_suggestion_data: String,
}

impl Translation {
    /// Parse strict data and validate named placeholders against the typed contract.
    pub(super) fn parse(source: &str) -> Result<Self, String> {
        let translation: Self = serde_json::from_str(source).map_err(|error| error.to_string())?;
        for (template, expected) in [
            (translation.flow_workspace.as_str(), &[][..]),
            (translation.flow_language.as_str(), &[][..]),
            (translation.flow_tone.as_str(), &[][..]),
            (translation.flow_settings.as_str(), &[][..]),
            (translation.flow_review.as_str(), &[][..]),
            (translation.flow_editor_prompt.as_str(), &[][..]),
            (translation.flow_apply_prompt.as_str(), &[][..]),
            (translation.flow_preview_prompt.as_str(), &[][..]),
            (translation.interface_fallback.as_str(), &["language"][..]),
            (translation.init_untrusted.as_str(), &["instruction"][..]),
            (translation.init_trust_command.as_str(), &["path"][..]),
            (translation.init_trust_data.as_str(), &["path"][..]),
            (
                translation.init_confirm.as_str(),
                &["failure", "instruction"][..],
            ),
            (translation.init_approve_prompt.as_str(), &["path"][..]),
            (translation.init_confirm_command.as_str(), &["path"][..]),
            (translation.init_confirm_data.as_str(), &["path"][..]),
            (translation.init_preferences_written.as_str(), &[][..]),
            (translation.init_preferences_declined.as_str(), &[][..]),
        ]
        .into_iter()
        .chain(translation.migrated_templates())
        {
            if placeholders(template)? != expected.iter().copied().collect() {
                return Err("interface message placeholder set mismatch".to_owned());
            }
        }
        Ok(translation)
    }

    /// Templates added by the frozen existing-message migration.
    fn migrated_templates(&self) -> [(&str, &[&str]); 31] {
        [
            (self.catalog_missing_ownership.as_str(), &[]),
            (self.catalog_check_passed.as_str(), &["count"]),
            (self.catalog_codeowners_regular.as_str(), &["path"]),
            (self.diagnostic_path.as_str(), &["path", "error"]),
            (self.catalog_codeowners_missing.as_str(), &["path", "rule"]),
            (self.catalog_codeowners_passed.as_str(), &[]),
            (self.catalog_compiled.as_str(), &["digest"]),
            (self.catalog_output_filename.as_str(), &[]),
            (self.catalog_staging_changed.as_str(), &[]),
            (self.catalog_output_regular.as_str(), &["path"]),
            (self.catalog_index_passed.as_str(), &[]),
            (self.catalog_evidence_filename.as_str(), &[]),
            (self.catalog_evidence_limit.as_str(), &["path"]),
            (self.catalog_owners_passed.as_str(), &[]),
            (self.catalog_target_untrusted.as_str(), &["instruction"]),
            (self.catalog_target_approve.as_str(), &["path"]),
            (
                self.diagnostic_instruction.as_str(),
                &["error", "instruction"],
            ),
            (self.catalog_target_declined.as_str(), &["instruction"]),
            (self.output_write_failed.as_str(), &["error"]),
            (self.run_repair_dispatch.as_str(), &[]),
            (self.run_retrieval_dispatch.as_str(), &[]),
            (self.run_prepare_models_off.as_str(), &[]),
            (self.run_publish_models_off.as_str(), &[]),
            (self.run_model_dispatch.as_str(), &[]),
            (self.session_workspace_directory.as_str(), &["path"]),
            (self.session_backend_unavailable.as_str(), &[]),
            (self.trust_home_unavailable.as_str(), &[]),
            (self.trust_approve.as_str(), &["path"]),
            (self.trust_recorded.as_str(), &["path"]),
            (self.trust_suggestion_command.as_str(), &["path"]),
            (self.trust_suggestion_data.as_str(), &["path"]),
        ]
    }

    /// Select wording before any data is interpolated.
    fn template(&self, key: MessageKey) -> &str {
        match key {
            MessageKey::FlowWorkspace => &self.flow_workspace,
            MessageKey::FlowLanguage => &self.flow_language,
            MessageKey::FlowTone => &self.flow_tone,
            MessageKey::FlowSettings => &self.flow_settings,
            MessageKey::FlowReview => &self.flow_review,
            MessageKey::FlowEditorPrompt => &self.flow_editor_prompt,
            MessageKey::FlowApplyPrompt => &self.flow_apply_prompt,
            MessageKey::FlowPreviewPrompt => &self.flow_preview_prompt,
            MessageKey::InterfaceFallback => &self.interface_fallback,
            MessageKey::InitUntrusted => &self.init_untrusted,
            MessageKey::InitTrustCommand => &self.init_trust_command,
            MessageKey::InitTrustData => &self.init_trust_data,
            MessageKey::InitConfirm => &self.init_confirm,
            MessageKey::InitApprovePrompt => &self.init_approve_prompt,
            MessageKey::InitConfirmCommand => &self.init_confirm_command,
            MessageKey::InitConfirmData => &self.init_confirm_data,
            MessageKey::InitPreferencesWritten => &self.init_preferences_written,
            MessageKey::InitPreferencesDeclined => &self.init_preferences_declined,
            MessageKey::CatalogMissingOwnership => &self.catalog_missing_ownership,
            MessageKey::CatalogCheckPassed => &self.catalog_check_passed,
            MessageKey::CatalogCodeownersRegular => &self.catalog_codeowners_regular,
            MessageKey::DiagnosticPath => &self.diagnostic_path,
            MessageKey::CatalogCodeownersMissing => &self.catalog_codeowners_missing,
            MessageKey::CatalogCodeownersPassed => &self.catalog_codeowners_passed,
            MessageKey::CatalogCompiled => &self.catalog_compiled,
            MessageKey::CatalogOutputFilename => &self.catalog_output_filename,
            MessageKey::CatalogStagingChanged => &self.catalog_staging_changed,
            MessageKey::CatalogOutputRegular => &self.catalog_output_regular,
            MessageKey::CatalogIndexPassed => &self.catalog_index_passed,
            MessageKey::CatalogEvidenceFilename => &self.catalog_evidence_filename,
            MessageKey::CatalogEvidenceLimit => &self.catalog_evidence_limit,
            MessageKey::CatalogOwnersPassed => &self.catalog_owners_passed,
            MessageKey::CatalogTargetUntrusted => &self.catalog_target_untrusted,
            MessageKey::CatalogTargetApprove => &self.catalog_target_approve,
            MessageKey::DiagnosticInstruction => &self.diagnostic_instruction,
            MessageKey::CatalogTargetDeclined => &self.catalog_target_declined,
            MessageKey::OutputWriteFailed => &self.output_write_failed,
            MessageKey::RunRepairDispatch => &self.run_repair_dispatch,
            MessageKey::RunRetrievalDispatch => &self.run_retrieval_dispatch,
            MessageKey::RunPrepareModelsOff => &self.run_prepare_models_off,
            MessageKey::RunPublishModelsOff => &self.run_publish_models_off,
            MessageKey::RunModelDispatch => &self.run_model_dispatch,
            MessageKey::SessionWorkspaceDirectory => &self.session_workspace_directory,
            MessageKey::SessionBackendUnavailable => &self.session_backend_unavailable,
            MessageKey::TrustHomeUnavailable => &self.trust_home_unavailable,
            MessageKey::TrustApprove => &self.trust_approve,
            MessageKey::TrustRecorded => &self.trust_recorded,
            MessageKey::TrustSuggestionCommand => &self.trust_suggestion_command,
            MessageKey::TrustSuggestionData => &self.trust_suggestion_data,
        }
    }
}

/// Shared, strictly validated built-in translations, never workspace-supplied text.
static TRANSLATIONS: OnceLock<Result<Vec<Translation>, String>> = OnceLock::new();

/// Interface-only selection; conversation language and tone are never rewritten.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Interface {
    /// The selected validated wording, independent of conversational tone.
    translation: &'static Translation,
    /// Whether the selected conversation tag needs an English-interface note.
    fallback: bool,
}

impl Interface {
    /// Select a supported tag or English, without any per-language branch.
    pub(crate) fn select(language: &str) -> Result<Self, String> {
        let translations = TRANSLATIONS
            .get_or_init(|| {
                BUILT_INS
                    .iter()
                    .map(|(_, source)| Translation::parse(source))
                    .collect()
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let selected = BUILT_INS.iter().position(|(tag, _)| *tag == language);
        Ok(Self {
            translation: translations
                .get(selected.unwrap_or(0))
                .ok_or_else(|| "missing built-in interface translation".to_owned())?,
            fallback: selected.is_none() && language != "auto",
        })
    }

    /// Whether to display the one English-interface note on the human boundary.
    pub(crate) const fn fallback(self) -> bool {
        self.fallback
    }

    /// Return only the template; interpolation is a separate operation.
    pub(crate) fn template(self, key: MessageKey) -> &'static str {
        self.translation.template(key)
    }
}

/// Extract strict ASCII named placeholders, rejecting malformed brace syntax.
fn placeholders(template: &str) -> Result<BTreeSet<&str>, String> {
    let mut rest = template;
    let mut names = BTreeSet::new();
    while let Some((before, after)) = rest.split_once('{') {
        if before.contains('}') {
            return Err("stray closing brace in interface message".to_owned());
        }
        let (name, tail) = after
            .split_once('}')
            .ok_or_else(|| "unclosed interface placeholder".to_owned())?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        {
            return Err("invalid named interface placeholder".to_owned());
        }
        names.insert(name);
        rest = tail;
    }
    if rest.contains('}') {
        return Err("stray closing brace in interface message".to_owned());
    }
    Ok(names)
}

/// Insert exact data values once; inserted braces never become new placeholders.
/// Missing, duplicate and extra arguments refuse rather than truncating a warning.
pub(crate) fn interpolate(template: &str, values: &[(&str, &str)]) -> Result<String, String> {
    let names: BTreeSet<_> = values.iter().map(|(name, _)| *name).collect();
    if names.len() != values.len() || names != placeholders(template)? {
        return Err("interface message arguments do not match placeholders".to_owned());
    }
    let mut rendered = String::new();
    let mut rest = template;
    while let Some((before, after)) = rest.split_once('{') {
        let (name, tail) = after
            .split_once('}')
            .ok_or_else(|| "unclosed interface placeholder".to_owned())?;
        let (_, value) = values
            .iter()
            .find(|(key, _)| *key == name)
            .ok_or_else(|| "missing interface argument".to_owned())?;
        rendered.push_str(before);
        rendered.push_str(value);
        rest = tail;
    }
    rendered.push_str(rest);
    Ok(rendered)
}
