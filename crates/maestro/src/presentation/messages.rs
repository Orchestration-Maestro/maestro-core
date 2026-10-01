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
#[derive(Clone, Copy)]
pub(crate) enum MessageKey {
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
}

/// Required keys; serde refuses missing, duplicate, mistyped and unknown fields.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Translation {
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
}

impl Translation {
    /// Parse strict data and validate named placeholders against the typed contract.
    pub(super) fn parse(source: &str) -> Result<Self, String> {
        let translation: Self = serde_json::from_str(source).map_err(|error| error.to_string())?;
        for (template, expected) in [
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
        ] {
            if placeholders(template)? != expected.iter().copied().collect() {
                return Err("interface message placeholder set mismatch".to_owned());
            }
        }
        Ok(translation)
    }

    /// Select wording before any data is interpolated.
    fn template(&self, key: MessageKey) -> &str {
        match key {
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
