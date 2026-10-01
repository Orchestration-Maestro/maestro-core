//! Decisions retain a no-follow parent capability; no file effect uses a reopened path.
use super::port::{CheckedTrust, TrustBoundaries, WorkspaceTrust as _};
use maestro_filesystem::Directory;
use std::{
    fs::File,
    io,
    path::{Component, Path, PathBuf},
};

/// Effect class requested by a Maestro-owned operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Non-secret reads remain eligible outside trust.
    Read,
    /// Writes require a checked, containing user-approved root.
    Write,
}

/// One validated leaf under a retained no-follow parent, never a broad directory grant.
#[derive(Debug)]
pub struct AuthorizedPath<'a> {
    /// Checked authority and immutable composition-root bindings, not a cached allow decision.
    trust: CheckedTrust<'a>,
    /// Caller spelling retained across canonicalization, including secret-looking aliases.
    supplied_base: PathBuf,
    /// Normal relative components as supplied by the operation.
    relative: PathBuf,
    /// Walked spelling before resolving the existing final leaf.
    walked_target: PathBuf,
    /// Parent held for the lifetime of the capability.
    parent: Directory,
    /// One normal UTF-8 name; no path separator or traversal is accepted.
    name: String,
    /// The decision cannot be promoted from read to write.
    access: Access,
}

impl<'a> CheckedTrust<'a> {
    /// Decide over a caller-selected root resolved once, and normal relative names.
    /// C05j must supply the operation's own declared root, not an attacker-selected
    /// target parent, and revalidate the retained capability immediately before use.
    /// Callers must pass base and relative separately, never join user input onto a
    /// verbatim base before this policy (Windows normalizes dot components there).
    /// Missing parents are refused; this decision creates nothing.
    ///
    /// # Errors
    /// Refuses traversal, links/reparse points, failed opens, secret/internal locations
    /// and outside writes. Ordinary outside reads remain subject to other controls.
    pub fn authorize(
        &self,
        base: &Path,
        relative: &Path,
        access: Access,
    ) -> io::Result<AuthorizedPath<'a>> {
        if base.components().any(|part| part == Component::ParentDir) {
            return Err(io::Error::other("base contains parent traversal"));
        }
        let names = relative_names(relative)?;
        check_supplied(self.boundaries, base, &names)?;
        let (name, parents) = names
            .split_last()
            .ok_or_else(|| io::Error::other("path has no leaf"))?;
        let resolved = base.canonicalize()?;
        let mut parent = Directory::open_canonical(&resolved)?;
        let mut canonical = parent.canonical_path()?;
        self.boundaries.check_target(&canonical)?;
        for name in parents {
            let walked = canonical.join(name);
            self.boundaries.check_target(&walked)?;
            parent = parent.child(name)?;
            canonical = parent.canonical_path()?;
            self.boundaries.check_target(&canonical)?;
        }
        let walked = canonical.join(name);
        self.boundaries.check_target(&walked)?;
        let authorized = AuthorizedPath {
            trust: Self::new(self.adapter, self.boundaries),
            supplied_base: base.to_path_buf(),
            relative: relative.to_path_buf(),
            walked_target: walked,
            parent,
            name: (*name).to_owned(),
            access,
        };
        authorized.revalidate()?;
        Ok(authorized)
    }
}

impl AuthorizedPath<'_> {
    /// Verify that the held parent still has its checked name and the leaf is eligible.
    ///
    /// # Errors
    /// Refuses replaced parents, linked/non-regular leaves and failed reads. Only a
    /// write decision permits an absent leaf; it never permits absent parents.
    pub fn revalidate(&self) -> io::Result<()> {
        self.check_current_policy()?;
        self.parent.verify_named()?;
        match self.parent.open_regular(&self.name) {
            Ok(file) => drop(file),
            Err(error)
                if self.access == Access::Write && error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.parent.verify_named()
    }

    /// Open the authorized regular file through its held parent, never through a link.
    ///
    /// # Errors
    /// Refuses write-only capabilities, swaps, links, non-regular and unreadable files.
    pub fn open_read(&self) -> io::Result<File> {
        if self.access != Access::Read {
            return Err(io::Error::other("not a read capability"));
        }
        self.revalidate()?;
        self.parent.open_regular(&self.name)
    }

    /// Test-only scheduling hook between the lease check and exclusive creation.
    #[cfg(test)]
    pub(crate) fn create_new_with(
        &self,
        after_check: impl FnOnce(),
        after_create: impl FnOnce(),
    ) -> io::Result<File> {
        self.prepare_creation()?;
        after_check();
        let file = self.parent.create_new(&self.name)?;
        after_create();
        self.finish_creation(file)
    }

    /// Create only the authorized absent leaf; no existing bytes are replaced.
    /// On Unix a same-user racer may briefly see the new file before rollback.
    /// A moved parent causes unlink through the held parent and refusal; a cleanup
    /// I/O failure is reported and can leave the new file behind. Rollback verifies
    /// the created identity and empty bytes, preserving replacements on mismatch.
    /// Windows holds all ancestors without delete-sharing, blocking the rename.
    ///
    /// # Errors
    /// Refuses read-only capabilities, swaps, existing names and failed creation.
    pub fn create_new(&self) -> io::Result<File> {
        self.prepare_creation()?;
        self.finish_creation(self.parent.create_new(&self.name)?)
    }

    /// A write lease rechecks current denies and journal authority immediately before creation.
    fn prepare_creation(&self) -> io::Result<()> {
        if self.access != Access::Write {
            return Err(io::Error::other("not a write capability"));
        }
        self.check_current_policy()?;
        self.parent.verify_named()
    }

    /// Do not return a created handle if its parent escaped after the check.
    fn finish_creation(&self, file: File) -> io::Result<File> {
        if let Err(error) = self.parent.verify_named() {
            self.rollback_creation(file).map_err(|cleanup| {
                io::Error::other(format!(
                    "parent changed: {error}; rollback refused; replacement not deleted: {cleanup}"
                ))
            })?;
            return Err(error);
        }
        Ok(file)
    }

    /// Delete only the empty object this operation created, never a replaced entry.
    fn rollback_creation(&self, file: File) -> io::Result<()> {
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt as _;
            let metadata = file.metadata()?;
            Some((metadata.dev(), metadata.ino()))
        };
        #[cfg(windows)]
        let identity = None;
        drop(file);
        self.parent.remove_verified(&self.name, &[], identity)
    }

    /// Refresh immutable deny data from live bindings, retaining both supplied and resolved names.
    fn check_current_policy(&self) -> io::Result<()> {
        let boundaries = self.trust.boundaries.refreshed()?;
        let names = relative_names(&self.relative)?;
        check_supplied(&boundaries, &self.supplied_base, &names)?;
        boundaries.check_target(&self.walked_target)?;
        let parent = self.parent.canonical_path()?;
        boundaries.check_target(&parent.join(&self.name))?;
        match parent.join(&self.name).canonicalize() {
            Ok(leaf) => {
                self.trust.boundaries.check_target(&leaf)?;
                boundaries.check_target(&leaf)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if self.access == Access::Write
            && CheckedTrust::new(&self.trust, &boundaries)
                .containing_root(&parent)
                .is_none()
        {
            return Err(io::Error::other(
                "write is outside current checked workspace trust",
            ));
        }
        self.parent.verify_named()
    }
}

/// Validate relative syntax before any platform path manipulation can erase components.
fn relative_names(relative: &Path) -> io::Result<Vec<&str>> {
    relative
        .components()
        .map(|part| {
            let Component::Normal(name) = part else {
                return Err(io::Error::other("relative path must contain normal names"));
            };
            let name = name
                .to_str()
                .ok_or_else(|| io::Error::other("path name is not UTF-8"))?;
            if name.contains(['\\', ':']) {
                return Err(io::Error::other("path name contains a separator or stream"));
            }
            Ok(name)
        })
        .collect()
}

/// Check every supplied component before canonicalization, never replacing it with its target.
fn check_supplied(boundaries: &TrustBoundaries, base: &Path, names: &[&str]) -> io::Result<()> {
    let mut supplied = base.to_path_buf();
    boundaries.check_target(&supplied)?;
    for name in names {
        supplied.push(name);
        boundaries.check_target(&supplied)?;
    }
    Ok(())
}
