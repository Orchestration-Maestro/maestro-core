//! Side-effect-free init preferences over S1's registry, parser and file adapter.
use super::resolve::resolve;
use crate::{
    files::{FileInput, FilePlan},
    limits::Limits,
};
use maestro_settings::{FileLayers, Layer, Layers, MAX_FILE_BYTES, Registry, Value, parse_flags};
use serde::Serialize;
use std::{collections::BTreeMap, fmt::Write as _, path::Path};

/// Replaceable source of strictly parsed preferences selected for the session.
pub trait WorkspacePreferences {
    /// Read preferences only, never user authority. The session selects the nearest
    /// safe workspace file and never merges ancestor configuration.
    ///
    /// # Errors
    /// Returns a named file, schema, registry or bounds refusal.
    fn layers(&self, registry: &Registry, limits: &Limits) -> Result<Layers, String>;
}

/// The shared S1 file adapter, bound to the user preferences and explicit root.
#[derive(Debug)]
pub struct FilePreferences {
    /// Paths interpreted solely by the shared preferences parser.
    files: FileLayers,
}

impl FilePreferences {
    /// Bind user `preferences.toml` and root-local `.maestro/config.toml`.
    #[must_use]
    pub fn new(config_dir: &Path, root: &Path) -> Self {
        Self {
            files: FileLayers::new(config_dir, Some(root.join(".maestro/config.toml"))),
        }
    }
}

impl WorkspacePreferences for FilePreferences {
    fn layers(&self, registry: &Registry, limits: &Limits) -> Result<Layers, String> {
        self.files
            .preferences(registry, preference_bytes(limits), limits.source_depth)
            .map_err(|error| error.to_string())
    }
}

/// Validated root-local config bytes and their independent C04 owned-file plan.
/// This carries no persistence capability; C05j supplies the real trust guard.
#[derive(Debug, Serialize)]
pub struct PreferencesDraft {
    /// The exact config bytes for draft consumers, without local paths or authority.
    #[serde(skip)]
    pub file: FileInput,
    /// The side-effect-free, digest-bound C04 config plan.
    pub files: FilePlan,
    /// English diagnostics explaining ignored widening requests.
    pub diagnostics: Vec<String>,
}

/// Plan init's selected preferences with typed `KEY=VALUE` choices.
/// Language defaults to en only for init, not for absent preference layers.
/// Other descriptor keys are emitted under `[overrides]`, resolved by C17.
///
/// # Errors
/// Returns an invalid file/choice, locked key, bounds, or existing-file conflict.
pub fn draft_preferences(
    root: &Path,
    port: &dyn WorkspacePreferences,
    choices: &[String],
    limits: &Limits,
) -> Result<PreferencesDraft, String> {
    let registry = Registry::built_in().map_err(|error| error.to_string())?;
    let layers = port.layers(&registry, limits)?;
    let flags = parse_flags(&registry, choices).map_err(|error| error.to_string())?;
    if flags
        .iter()
        .any(|flag| flag.key == "language" && flag.value == Value::Text("auto".to_owned()))
    {
        return Err("init language must be a canonical language tag, not auto".to_owned());
    }
    let resolved = resolve(
        &registry,
        &maestro_settings::resolve(&registry, &layers, &flags),
    );
    let language = match resolved.text("language") {
        Some("auto") | None => "en",
        Some(language) => language,
    };
    let mut diagnostics: Vec<_> = resolved
        .diagnostics()
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.key, diagnostic.message))
        .collect();
    let updates = match resolved.text("updates") {
        Some("auto") => {
            diagnostics.push(
                "updates: ignored init auto request; only user preferences enable auto".to_owned(),
            );
            "propose"
        }
        Some(updates) => updates,
        None => "propose",
    };
    let mut text = format!(
        "schema = {:?}\nlanguage = {:?}\ntone = {:?}\nupdates = {:?}\n",
        maestro_settings::SCHEMA,
        language,
        resolved.text("tone").unwrap_or("normal"),
        updates
    );
    let mut overrides = BTreeMap::new();
    for flag in &flags {
        if matches!(flag.key.as_str(), "language" | "tone" | "updates") {
            continue;
        }
        let selected = resolved
            .get(&flag.key)
            .and_then(|value| value.as_ref().ok())
            .ok_or_else(|| format!("{}: setting cannot be resolved", flag.key))?;
        overrides.insert(&flag.key, selected.value());
    }
    if !overrides.is_empty() {
        text.push_str("\n[overrides]\n");
        for (key, value) in overrides {
            writeln!(text, "{key:?} = {}", value.to_toml()).map_err(|error| error.to_string())?;
        }
    }
    Layer::parse_preferences(
        &registry,
        &text,
        preference_bytes(limits),
        limits.source_depth,
    )
    .map_err(|error| error.to_string())?;
    let file = FileInput::new(".maestro/config.toml", text.into_bytes());
    let files = FilePlan::preview(root, [file.clone()]).map_err(|error| error.to_string())?;
    Ok(PreferencesDraft {
        file,
        files,
        diagnostics,
    })
}

/// Preserve S1's 64 KiB preferences ceiling while allowing smaller injected limits.
fn preference_bytes(limits: &Limits) -> u64 {
    limits.source_file_bytes.min(MAX_FILE_BYTES as u64)
}
