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
use std::{fmt, path::Path};

/// Nonfatal activation diagnostics retained independently of preference display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GraphActivationError {
    /// An existing verified lock selects the uncompiled native graph base.
    EngineMissing,
    /// A named trust, ownership, schema or settings refusal.
    Refused(String),
}
impl fmt::Display for GraphActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineMissing => formatter
                .write_str("graph.engine = ladybug, but this maestro was built without the engine"),
            Self::Refused(message) => formatter.write_str(message),
        }
    }
}

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
    /// Complete authoring-lock identity from read-only catalog admission.
    pub(crate) frozen_lock: Option<String>,
    /// A nonfatal health activation refusal; runtime sessions never carry one.
    pub(crate) graph_activation_error: Option<GraphActivationError>,
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
        let registry = source.registry().map_err(Failure::refused)?;
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
            frozen_lock: source.frozen_lock().map(str::to_owned),
            graph_activation_error: None,
        })
    }

    /// Path-free, immutable initialization provenance for model-visible MCP instructions.
    ///
    /// # Errors
    /// [`Failure::Failed`] for a missing or invalid registered presentation setting.
    pub(crate) fn mcp_context(&self) -> Result<String, Failure> {
        let fragment = settings::conversation_instructions(&self.catalog_resolved())
            .map_err(Failure::failed)?;
        let origin = if self.discovery.file.is_some() {
            "workspace-selected (explicit --workspace)"
        } else if self.discovery.note.is_some() {
            "user/default fallback; explicit workspace outside home or unavailable"
        } else if self.layers.user.is_some() {
            "user preferences; no workspace file selected"
        } else {
            "built-in defaults; no workspace file selected"
        };
        Ok(format!(
            "{fragment} Preferences: {origin}. Workspace overrides require --workspace. \
            Preferences are fixed for this session; restart for edits; \
            tool arguments cannot replace them."
        ))
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
    fn frozen_lock(&self) -> Option<&str> {
        self.frozen_lock.as_deref()
    }

    fn registry(&self) -> Result<Registry, String> {
        Ok(self.registry.clone())
    }

    fn layers(&self, _registry: &Registry, _limits: &Limits) -> Result<Layers, String> {
        Ok(self.layers.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_activation_diagnostics_preserve_the_reason() {
        assert_eq!(
            GraphActivationError::EngineMissing.to_string(),
            "graph.engine = ladybug, but this maestro was built without the engine"
        );
        assert_eq!(
            GraphActivationError::Refused("untrusted lock".into()).to_string(),
            "untrusted lock"
        );
    }

    /// Storage-free snapshot whose registry can exercise internal invariants.
    fn source(registry: Registry) -> Session {
        Session {
            registry,
            files: FileLayers::new(Path::new("config"), None),
            discovery: Discovery::default(),
            layers: Layers::default(),
            flags: Vec::new(),
            frozen_lock: None,
            graph_activation_error: None,
        }
    }

    #[test]
    fn catalog_client_preferences_missing_registered_language_is_failed() {
        let session = source(Registry::new(&[]).unwrap());
        assert!(matches!(session.mcp_context(), Err(Failure::Failed(key)) if key == "language"));
    }

    #[test]
    fn catalog_client_preferences_freezes_values_even_when_source_changes() {
        let mut port = source(Registry::built_in().unwrap());
        let session = Session::from_preferences(
            Path::new("config"),
            &port,
            Discovery::default(),
            &["language=JA".to_owned(), "tone=detailed".to_owned()],
        )
        .unwrap();
        let before = session.mcp_context().unwrap();
        port.registry = Registry::new(&[]).unwrap();
        assert_eq!(session.mcp_context().unwrap(), before);
        assert!(before.starts_with("Conversation language: \"ja\"; tone: \"detailed\"."));
    }
}
