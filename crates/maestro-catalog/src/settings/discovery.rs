//! Safe session snapshots over S1's bounded parser and discovery walk.
use super::preferences::WorkspacePreferences;
use crate::limits::Limits;
use maestro_filesystem::Directory;
use maestro_settings::{
    Discovery, FileLayers, Layer, Layers, MAX_FILE_BYTES, PROJECT_DIRECTORY, PROJECT_FILE,
    Registry, discover_project_with,
};
use std::{
    io,
    path::{Path, PathBuf},
};

/// Selected safe file bytes, including a fatal byte-limit refusal.
type PreferenceBytes = io::Result<Vec<u8>>;

pub use crate::policy::workspace::WorkspaceTrust;

/// Until C05h supplies journal records, external workspaces are never trusted.
#[derive(Debug)]
pub struct NoWorkspaceTrust;
impl WorkspaceTrust for NoWorkspaceTrust {
    fn containing_root(&self, _canonical_start: &Path) -> Option<PathBuf> {
        None
    }
}

/// An immutable per-process snapshot; edits become visible only to the next session.
#[derive(Debug)]
pub struct SessionPreferences {
    /// Selected file and local-only warnings/provenance.
    pub discovery: Discovery,
    /// Strictly parsed layers; no file is reopened during resolution.
    layers: Layers,
}

impl SessionPreferences {
    /// Load user preferences and at most one safe workspace file, before effects.
    /// No start (MCP without `--workspace`) means user preferences only.
    ///
    /// # Errors
    /// Refuses malformed selected safe files, including masked keys and byte/depth limits.
    pub fn load(
        config_dir: &Path,
        start: Option<&Path>,
        home: Option<&Path>,
        trust: &dyn WorkspaceTrust,
        limits: &Limits,
    ) -> Result<Self, String> {
        let registry = Registry::built_in().map_err(|error| error.to_string())?;
        let max_bytes = limits.source_file_bytes.min(MAX_FILE_BYTES as u64);
        let mut layers = FileLayers::new(config_dir, None)
            .preferences(&registry, max_bytes, limits.source_depth)
            .map_err(|error| error.to_string())?;
        let (discovery, project) = match start {
            Some(start) => discover(start, home, trust, max_bytes),
            None => (Discovery::default(), None),
        };
        if let (Some(path), Some(bytes)) = (&discovery.file, project) {
            let bytes = bytes.map_err(|error| format!("{}: {error}", path.display()))?;
            let text =
                String::from_utf8(bytes).map_err(|error| format!("{}: {error}", path.display()))?;
            let layer = Layer::parse_preferences(&registry, &text, max_bytes, limits.source_depth)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            layers.project = Some((path.clone(), layer));
        }
        Ok(Self { discovery, layers })
    }
}

impl WorkspacePreferences for SessionPreferences {
    fn layers(&self, _registry: &Registry, _limits: &Limits) -> Result<Layers, String> {
        Ok(self.layers.clone())
    }
}

/// Bound a canonical start by home or a containing journal-approved root.
fn discover(
    start: &Path,
    home: Option<&Path>,
    trust: &dyn WorkspaceTrust,
    max_bytes: u64,
) -> (Discovery, Option<io::Result<Vec<u8>>>) {
    let start = match start.canonicalize() {
        Ok(start) if start.is_dir() => start,
        _ => return noted("workspace cannot be resolved as a directory".to_owned()),
    };
    let home = home.and_then(|home| home.canonicalize().ok());
    let boundary = home.filter(|home| start.starts_with(home)).or_else(|| {
        trust
            .containing_root(&start)
            .filter(|root| root.is_absolute() && start.starts_with(root))
    });
    let Some(boundary) = boundary else {
        return noted(format!(
            "outside home without workspace trust; no workspace file read; maestro trust add {}",
            start.display()
        ));
    };
    let mut held = Directory::open_canonical(&start);
    discover_project_with(&start, &boundary, |directory| {
        let candidate = match &held {
            Ok(held) => candidate(held, directory, max_bytes),
            Err(error) => Err(format!("{}: skipped: {error}", directory.display())),
        };
        let selected = candidate.as_ref().is_ok_and(Option::is_some);
        if directory != boundary && !selected {
            held = match &held {
                Ok(directory) => directory.parent(),
                Err(error) => Err(io::Error::other(error.to_string())),
            };
        }
        candidate
    })
}

/// Read a candidate through the held workspace directory, never by reopening its file path.
fn candidate(
    directory: &Directory,
    path: &Path,
    max_bytes: u64,
) -> Result<Option<(PathBuf, PreferenceBytes)>, String> {
    let file = path.join(PROJECT_DIRECTORY).join(PROJECT_FILE);
    let read = || {
        if directory.is_mount_root()? {
            return Err(io::Error::other("mount or drive root is never a workspace"));
        }
        directory
            .child(PROJECT_DIRECTORY)?
            .read_preferences(PROJECT_FILE, max_bytes)
    };
    match read() {
        Ok(bytes) => Ok(Some((file, Ok(bytes)))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::FileTooLarge => Ok(Some((file, Err(error)))),
        Err(error) => Err(format!("{}: skipped: {error}", file.display())),
    }
}

/// Explain fallback without selecting or reading any workspace candidate.
fn noted(note: String) -> (Discovery, Option<io::Result<Vec<u8>>>) {
    (
        Discovery {
            note: Some(note),
            ..Discovery::default()
        },
        None,
    )
}
