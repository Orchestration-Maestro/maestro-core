//! Replaceable journal authority adapter inside mandatory root refusals.
use maestro_filesystem::Directory;
use maestro_kernel::workspace::WorkspaceAuthority;
use std::{
    fmt, io,
    path::{Path, PathBuf},
};

/// User-local authorization, never preference or catalog content.
pub trait WorkspaceTrust {
    /// Return a canonical containing root backed by user approval, or none.
    fn containing_root(&self, canonical_start: &Path) -> Option<PathBuf>;
}

/// Platform-resolved home and kernel-internal locations, supplied by the composition root.
#[derive(Debug)]
pub struct TrustBoundaries {
    /// Canonical HOME; its descendants remain independently eligible.
    home: PathBuf,
    /// Canonical internal directories; tools may never trust these subtrees.
    internal: Vec<PathBuf>,
}

impl TrustBoundaries {
    /// Resolve existing ancestors of platform bindings, without creating directories.
    ///
    /// # Errors
    /// Refuses unresolved home/internal locations rather than omitting a deny.
    pub fn new(home: &Path, internal: &[PathBuf]) -> io::Result<Self> {
        Ok(Self {
            home: home.canonicalize()?,
            internal: internal
                .iter()
                .map(|path| canonical_location(path))
                .collect::<io::Result<_>>()?,
        })
    }

    /// Resolve the selected directory once and apply the mandatory refusal floor.
    ///
    /// # Errors
    /// Refuses roots, HOME, internal directories and unresolved paths.
    pub fn canonical_root(&self, path: &Path) -> Result<PathBuf, String> {
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("cannot trust {}: {error}", path.display()))?;
        self.check_root(&canonical)?;
        Ok(canonical)
    }

    /// Check a canonical adapter result; no replacement adapter may bypass this floor.
    ///
    /// # Errors
    /// Refuses filesystem/drive/mount roots, HOME and kernel-internal directories.
    pub fn check_root(&self, canonical: &Path) -> Result<(), String> {
        let refuse = |reason: &str| format!("cannot trust {}: {reason}", canonical.display());
        if canonical == self.home {
            return Err(refuse("HOME itself"));
        }
        if self
            .internal
            .iter()
            .any(|internal| canonical.starts_with(internal))
        {
            return Err(refuse("kernel-internal directory"));
        }
        let held =
            Directory::open_canonical(canonical).map_err(|error| refuse(&error.to_string()))?;
        if held
            .is_mount_root()
            .map_err(|error| refuse(&error.to_string()))?
        {
            return Err(refuse("mount or drive root"));
        }
        Ok(())
    }
}

/// Resolve a possibly not-yet-created internal binding using its existing ancestor.
fn canonical_location(path: &Path) -> io::Result<PathBuf> {
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::other("internal path has no ancestor"))?;
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::other("internal path has no name"))?;
            Ok(canonical_location(parent)?.join(name))
        }
        Err(error) => Err(error),
    }
}

/// Read the existing kernel journal afresh, so revocation affects subsequent decisions.
pub struct JournalTrust<'a> {
    /// User-local kernel authority; no preference file is consulted.
    database: &'a dyn WorkspaceAuthority,
}

impl<'a> JournalTrust<'a> {
    /// Bind the replaceable default adapter to its user-local database.
    #[must_use]
    pub const fn new(database: &'a dyn WorkspaceAuthority) -> Self {
        Self { database }
    }
}

impl WorkspaceTrust for JournalTrust<'_> {
    fn containing_root(&self, canonical_start: &Path) -> Option<PathBuf> {
        self.database
            .read_trusted_workspaces()
            .ok()?
            .into_iter()
            .map(|record| record.change.path)
            .filter(|root| canonical_start.starts_with(root))
            .max_by_key(|root| root.components().count())
    }
}

/// Non-overridable refusal wrapper around any authority adapter.
pub struct CheckedTrust<'a> {
    /// Replaceable source of approved roots.
    adapter: &'a dyn WorkspaceTrust,
    /// Mandatory refusal floor.
    boundaries: &'a TrustBoundaries,
}

impl<'a> CheckedTrust<'a> {
    /// Wrap the adapter; consumers receive only the checked port.
    #[must_use]
    pub const fn new(adapter: &'a dyn WorkspaceTrust, boundaries: &'a TrustBoundaries) -> Self {
        Self {
            adapter,
            boundaries,
        }
    }
}

impl WorkspaceTrust for CheckedTrust<'_> {
    fn containing_root(&self, canonical_start: &Path) -> Option<PathBuf> {
        let root = self.adapter.containing_root(canonical_start)?;
        if !canonical_start.starts_with(&root) || self.boundaries.check_root(&root).is_err() {
            return None;
        }
        Some(root)
    }
}

impl fmt::Debug for CheckedTrust<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedTrust")
            .field("boundaries", &self.boundaries)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for JournalTrust<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JournalTrust")
            .finish_non_exhaustive()
    }
}
