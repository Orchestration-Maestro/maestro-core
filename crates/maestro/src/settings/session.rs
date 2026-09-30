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
    settings::{
        self, NoWorkspaceTrust, ResolvedSettings, SessionPreferences, WorkspacePreferences,
    },
};
use maestro_kernel::paths::{self, Environment};
use maestro_settings::{
    Discovery, FileLayers, Flag, Layers, Registry, Resolved, parse_flags, resolve,
};
use std::{
    env,
    path::{Path, PathBuf},
};

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
    /// The command line's session: the user file, the project file found
    /// from the working directory, and `flags`.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] naming a file or flag that is refused, and
    /// [`Failure::Failed`] when the configuration directory cannot be found.
    pub(crate) fn for_cli(flags: &[String]) -> Result<Self, Failure> {
        let start = env::current_dir().ok();
        Self::at(
            &config_dir()?,
            start.as_deref(),
            env::home_dir().as_deref(),
            flags,
        )
    }

    /// The MCP server's session: the user file, the project file found from
    /// `workspace` when one is given, and `flags`.
    ///
    /// # Errors
    ///
    /// As [`Session::for_cli`], and [`Failure::Refused`] for a workspace
    /// whose project file cannot be read, outside home among others.
    pub(crate) fn for_mcp(workspace: Option<&Path>, flags: &[String]) -> Result<Self, Failure> {
        Self::for_mcp_at(&config_dir()?, workspace, env::home_dir().as_deref(), flags)
    }

    /// [`Session::for_mcp`] with the directories given.
    pub(crate) fn for_mcp_at(
        config_dir: &Path,
        workspace: Option<&Path>,
        home: Option<&Path>,
        flags: &[String],
    ) -> Result<Self, Failure> {
        if let Some(workspace) = workspace
            && !workspace.is_dir()
        {
            return Err(Failure::refused(format!(
                "--workspace {}: the path is not a directory: no project file is read",
                workspace.display()
            )));
        }
        Self::at(config_dir, workspace, home, flags)
    }

    /// The session of the user file in `config_dir`, the project file found
    /// from `start` within `home` (none without a start), and `flags`.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] naming a file or flag that is refused.
    pub(crate) fn at(
        config_dir: &Path,
        start: Option<&Path>,
        home: Option<&Path>,
        flags: &[String],
    ) -> Result<Self, Failure> {
        let registry = Registry::built_in().map_err(|error| Failure::failed_by(&error))?;
        let snapshot = SessionPreferences::load(
            config_dir,
            start,
            home,
            &NoWorkspaceTrust,
            &Limits::PRODUCTION,
        )
        .map_err(Failure::refused)?;
        let layers = snapshot
            .layers(&registry, &Limits::PRODUCTION)
            .map_err(Failure::refused)?;
        let discovery = snapshot.discovery;
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

/// The kernel's configuration directory, which holds the user file.
fn config_dir() -> Result<PathBuf, Failure> {
    paths::config_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))
}
