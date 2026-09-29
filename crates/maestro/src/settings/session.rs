//! A session's settings: the registry, the files it reads and the explicit
//! `--set` flags, resolved once before any effect (plan D6). The command
//! line discovers its project file upward from its working directory; the
//! MCP server reads one only through an explicit `--workspace`, never its
//! working directory or a client's roots. Both resolve through this one
//! place, so they see the same values.

use super::knowledge::KnowledgeSettings;
use crate::failure::Failure;
use maestro_kernel::paths::{self, Environment};
use maestro_settings::{
    Discovery, FileLayers, Flag, LayerSource as _, Layers, Registry, Resolved,
    discover_project_file, parse_flags, resolve,
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
        let session = Self::at(config_dir, workspace, home, flags)?;
        if let (Some(workspace), Some(note)) = (workspace, &session.discovery.note) {
            return Err(Failure::refused(format!(
                "--workspace {}: {note}",
                workspace.display()
            )));
        }
        Ok(session)
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
        let discovery = start.map_or_else(
            || Discovery {
                note: Some("no working directory to start from".to_owned()),
                ..Discovery::default()
            },
            |start| discover_project_file(start, home),
        );
        let files = FileLayers::new(config_dir, discovery.file.clone());
        let flags = parse_flags(&registry, flags).map_err(|error| Failure::refused_by(&error))?;
        let layers = files
            .layers(&registry)
            .map_err(|error| Failure::refused_by(&error))?;
        Ok(Self {
            registry,
            files,
            discovery,
            layers,
            flags,
        })
    }

    /// Every setting's effective value, with the layer that set it.
    pub(crate) fn resolved(&self) -> Resolved<'_> {
        resolve(&self.registry, &self.layers, &self.flags)
    }

    /// The knowledge operations' settings.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when a registered setting is missing or of
    /// another kind than its consumer reads, which the registry's tests
    /// rule out.
    pub(crate) fn knowledge(&self) -> Result<KnowledgeSettings, Failure> {
        KnowledgeSettings::from_resolved(&self.resolved()).map_err(Failure::failed)
    }
}

/// The kernel's configuration directory, which holds the user file.
fn config_dir() -> Result<PathBuf, Failure> {
    paths::config_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))
}
