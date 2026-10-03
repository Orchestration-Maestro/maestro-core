//! Replaceable journal authority adapter inside mandatory root refusals.
use super::deny::{SecretPaths, canonical_location, contains};
use maestro_filesystem::Directory;
use maestro_kernel::{paths::Environment, workspace::WorkspaceAuthority};
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
    /// Immutable secret denies shared by every checked adapter.
    secrets: SecretPaths,
    /// Composition-root HOME spelling, retained for effect-time alias resolution.
    home_binding: PathBuf,
    /// Composition-root internal bindings, never supplied by catalog content.
    internal_bindings: Vec<PathBuf>,
    /// Immutable per-composition platform variables; filesystem aliases stay live.
    environment: Environment,
}

impl TrustBoundaries {
    /// Resolve existing ancestors of platform bindings, without creating directories.
    ///
    /// # Errors
    /// Refuses unresolved home/internal locations rather than omitting a deny.
    pub fn new(home: &Path, internal: &[PathBuf]) -> io::Result<Self> {
        Self::with_environment(home, internal, &Environment::current())
    }

    /// Resolve composition-root platform bindings; workspace data never supplies them.
    ///
    /// # Errors
    /// Refuses unresolved locations or malformed immutable deny data.
    pub fn with_environment(
        home: &Path,
        internal: &[PathBuf],
        environment: &Environment,
    ) -> io::Result<Self> {
        let home_binding = home.to_path_buf();
        let home = Directory::open_canonical(&home.canonicalize()?)?.canonical_path()?;
        Ok(Self {
            home_binding,
            internal_bindings: internal.to_vec(),
            environment: environment.clone(),
            secrets: SecretPaths::built_in(&home, environment)?,
            home,
            internal: internal
                .iter()
                .map(|path| canonical_location(path))
                .collect::<io::Result<_>>()?,
        })
    }

    /// Re-resolve the same trusted bindings against the current filesystem before an effect.
    pub(super) fn refreshed(&self) -> io::Result<Self> {
        Self::with_environment(
            &self.home_binding,
            &self.internal_bindings,
            &self.environment,
        )
    }

    /// Mandatory target denies, including when a broader directory was approved.
    pub(super) fn check_target(&self, path: &Path) -> io::Result<()> {
        if self.secrets.refuses(path)
            || self
                .internal
                .iter()
                .any(|internal| contains(internal, path))
        {
            return Err(io::Error::other("secret or kernel-internal location"));
        }
        Ok(())
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

/// Read the existing kernel journal afresh, so revocation affects subsequent decisions.
pub struct JournalTrust<'a> {
    /// User-local kernel authority; no preference file is consulted.
    database: Option<&'a dyn WorkspaceAuthority>,
}

impl<'a> JournalTrust<'a> {
    /// Bind the replaceable default adapter to its user-local database.
    #[must_use]
    pub const fn new(database: &'a dyn WorkspaceAuthority) -> Self {
        Self {
            database: Some(database),
        }
    }
    /// Preview can consult an existing journal without creating one for an unapproved workspace.
    #[must_use]
    pub const fn optional(database: Option<&'a dyn WorkspaceAuthority>) -> Self {
        Self { database }
    }
}

impl WorkspaceTrust for JournalTrust<'_> {
    fn containing_root(&self, canonical_start: &Path) -> Option<PathBuf> {
        self.database?
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
    pub(super) adapter: &'a dyn WorkspaceTrust,
    /// Mandatory refusal floor.
    pub(super) boundaries: &'a TrustBoundaries,
    /// Only a consumed, exact-path preferences confirmation can populate this narrow exception.
    pub(super) preferences: Option<PathBuf>,
}

impl<'a> CheckedTrust<'a> {
    /// Wrap the adapter; consumers receive only the checked port.
    #[must_use]
    pub const fn new(adapter: &'a dyn WorkspaceTrust, boundaries: &'a TrustBoundaries) -> Self {
        Self {
            adapter,
            boundaries,
            preferences: None,
        }
    }

    /// A private single-config effect lease, never a containing-root grant.
    pub(super) fn preferences(
        adapter: &'a dyn WorkspaceTrust,
        boundaries: &'a TrustBoundaries,
        root: &Path,
    ) -> Result<Self, String> {
        boundaries.check_root(root)?;
        Ok(Self {
            adapter,
            boundaries,
            preferences: Some(root.to_path_buf()),
        })
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
