//! A session's settings: the registry, the files it reads and the explicit
//! `--set` flags, resolved once before any effect (plan D6). The command
//! line discovers its project file upward from its working directory; the
//! MCP server reads one only through an explicit `--workspace`, never its
//! working directory or a client's roots. Both resolve through this one
//! place, so they see the same values.

use super::knowledge::KnowledgeSettings;
use crate::failure::Failure;
use maestro_catalog::{
    limits::Limits,
    settings::{self, ResolvedSettings, WorkspacePreferences},
};
use maestro_settings::{
    Discovery, FileLayers, Flag, Layers, Registry, Resolved, parse_flags, resolve,
};
use std::path::Path;

/// One session's settings.
#[derive(Debug)]
pub(crate) struct Session {
    /// The settings Maestro knows.
    pub(crate) registry: Registry,
    /// The files the session reads.
    pub(crate) files: FileLayers,
    /// What project discovery found, for `config explain` and `doctor`.
    pub(crate) discovery: Discovery,
    /// The files' layers.
    pub(crate) layers: Layers,
    /// The explicit `--set` flags.
    pub(crate) flags: Vec<Flag>,
}

impl Session {
    /// Resolve the injected source once with its discovery metadata and flags.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] naming a refused preference file or flag.
    pub(crate) fn from_preferences(
        config_dir: &Path,
        source: &dyn WorkspacePreferences,
        discovery: Discovery,
        flags: &[String],
    ) -> Result<Self, Failure> {
        let registry = Registry::built_in().map_err(|error| Failure::failed_by(&error))?;
        let layers = source
            .layers(&registry, &Limits::PRODUCTION)
            .map_err(Failure::refused)?;
        if let Some(note) = &discovery.note {
            eprintln!("{note}");
        }
        for skipped in &discovery.skipped {
            eprintln!("{skipped}");
        }
        let files = FileLayers::new(config_dir, discovery.file.clone());
        let flags = parse_flags(&registry, flags).map_err(|error| Failure::refused_by(&error))?;
        Ok(Self {
            registry,
            files,
            discovery,
            layers,
            flags,
        })
    }

    /// Path-free, immutable initialization provenance for model-visible MCP instructions.
    pub(crate) fn mcp_context(&self) -> String {
        let origin = if self.discovery.file.is_some() {
            "workspace-selected (explicit --workspace)"
        } else if self.discovery.note.is_some() {
            "user/default fallback; explicit workspace outside home or unavailable"
        } else if self.layers.user.is_some() {
            "user preferences; no workspace file selected"
        } else {
            "built-in defaults; no workspace file selected"
        };
        format!(
            "Preferences: {origin}. Workspace overrides require --workspace. \
            Preferences are fixed for this session; restart for edits; \
            tool arguments cannot replace them."
        )
    }

    /// Every setting's effective value, with the layer that set it.
    pub(crate) fn resolved(&self) -> Resolved<'_> {
        resolve(&self.registry, &self.layers, &self.flags)
    }

    /// Applies catalog restrictions to S1's already parsed values and provenance.
    pub(crate) fn catalog_resolved(&self) -> ResolvedSettings {
        settings::resolve(&self.registry, &self.resolved())
    }

    /// The knowledge operations' settings.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when a registered setting is missing or of
    /// another kind than its consumer reads, which the registry's tests
    /// rule out. Invalid user combinations are [`Failure::Refused`].
    pub(crate) fn knowledge(&self) -> Result<KnowledgeSettings, Failure> {
        let mut settings =
            KnowledgeSettings::from_resolved(&self.resolved()).map_err(Failure::failed)?;
        if let Some(output_tokens) = self
            .catalog_resolved()
            .integer("ask.output_tokens")
            .and_then(|value| u32::try_from(value).ok())
        {
            settings.ask_budget.output_tokens = Some(output_tokens);
        }
        settings.evidence.validate().map_err(Failure::refused)?;
        Ok(settings)
    }
}

impl WorkspacePreferences for Session {
    fn layers(&self, _registry: &Registry, _limits: &Limits) -> Result<Layers, String> {
        Ok(self.layers.clone())
    }
}
