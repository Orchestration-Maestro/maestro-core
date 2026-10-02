//! Decisions retain a no-follow parent capability; no file effect uses a reopened path.
use super::port::{CheckedTrust, TrustBoundaries, WorkspaceTrust as _};
use maestro_filesystem::{Directory, PublicationChecks};
use std::{
    fs::{File, Metadata},
    io::{self, Write as _},
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
        self.resolve_effect(base, relative, access, ParentMode::Existing)
    }

    /// Check a complete write target before creating any directory or journal.
    ///
    /// # Errors
    /// Refuses denied paths and existing linked parents; absent parents create nothing.
    pub fn check_write(&self, base: &Path, relative: &Path) -> io::Result<()> {
        self.resolve_effect(base, relative, Access::Write, ParentMode::Preview)?;
        Ok(())
    }

    /// Authorize one new file, creating missing parents through this same checked port.
    ///
    /// # Errors
    /// Refuses policy changes, links, outside targets and failed directory creation/rollback.
    pub fn authorize_create(&self, base: &Path, relative: &Path) -> io::Result<AuthorizedPath<'a>> {
        self.check_write(base, relative)?;
        self.resolve_effect(base, relative, Access::Write, ParentMode::Create)
    }

    /// The one no-follow walker serves previews, reads and guarded parent creation.
    fn resolve_effect(
        &self,
        base: &Path,
        relative: &Path,
        access: Access,
        mode: ParentMode,
    ) -> io::Result<AuthorizedPath<'a>> {
        if base.components().any(|part| part == Component::ParentDir) {
            return Err(io::Error::other("base contains parent traversal"));
        }
        let names = relative_names(relative)?;
        check_supplied(self.boundaries, base, &names)?;
        let (leaf, parents) = names
            .split_last()
            .ok_or_else(|| io::Error::other("path has no leaf"))?;
        let resolved = base.canonicalize()?;
        let mut parent = Directory::open_canonical(&resolved)?;
        let mut canonical = parent.canonical_path()?;
        check_supplied(self.boundaries, &canonical, &names)?;
        for name in parents {
            let walked = canonical.join(name);
            let candidate = self.retained((base, relative), (walked, name), parent, access);
            parent = match candidate.parent.child(name) {
                Ok(child) => child,
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound && mode == ParentMode::Create =>
                {
                    candidate.create_directory()?
                }
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound && mode == ParentMode::Preview =>
                {
                    let mut candidate = candidate;
                    candidate.walked_target = names
                        .iter()
                        .fold(canonical.clone(), |path, name| path.join(name));
                    candidate.check_current_policy()?;
                    return Ok(candidate);
                }
                Err(error) => return Err(error),
            };
            canonical = parent.canonical_path()?;
            self.boundaries.check_target(&canonical)?;
        }
        let authorized = self.retained(
            (base, relative),
            (canonical.join(leaf), leaf),
            parent,
            access,
        );
        authorized.revalidate()?;
        Ok(authorized)
    }

    /// Bind the operation's original spelling to the retained parent, not its canonical alias.
    fn retained(
        &self,
        (base, relative): (&Path, &Path),
        (walked_target, name): (PathBuf, &str),
        parent: Directory,
        access: Access,
    ) -> AuthorizedPath<'a> {
        AuthorizedPath {
            trust: Self {
                adapter: self.adapter,
                boundaries: self.boundaries,
                preferences: self.preferences.clone(),
            },
            supplied_base: base.to_path_buf(),
            relative: relative.to_path_buf(),
            walked_target,
            parent,
            name: name.to_owned(),
            access,
        }
    }
}

/// Missing-parent behavior is selected only by the effect port, never by operation content.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParentMode {
    /// Reads and removal require an existing parent.
    Existing,
    /// Preflight accepts missing parents but creates nothing.
    Preview,
    /// Each missing parent is a separately checked directory effect.
    Create,
}

impl AuthorizedPath<'_> {
    /// Create one directory, with an effect-time floor and same-identity empty rollback.
    fn create_directory(&self) -> io::Result<Directory> {
        self.create_directory_with(|| {}, || {})
    }

    /// Internal scheduling seam reaches native rollback without a production override.
    fn create_directory_with(
        &self,
        before: impl FnOnce(),
        after: impl FnOnce(),
    ) -> io::Result<Directory> {
        self.prepare_creation()?;
        before();
        let child = self.parent.create_child(&self.name)?;
        after();
        match self
            .check_current_policy()
            .and_then(|()| self.parent.harden_created_child(&self.name, &child))
        {
            Ok(hardened) => Ok(hardened),
            Err(error) => {
                self.parent
                    .remove_created_child(&self.name, &child)
                    .map_err(|cleanup| {
                        io::Error::other(format!("{error}; rollback failed: {cleanup}"))
                    })?;
                Err(error)
            }
        }
    }

    /// Create, write and sync one absent leaf as one checked effect; no File grant escapes.
    ///
    /// # Errors
    /// Refuses changed policy and rolls back only the created identity and written bytes.
    pub fn write_new(&self, bytes: &[u8]) -> io::Result<Metadata> {
        self.write_new_with(bytes, || {})
    }

    /// Scheduling seam used by owned-effect tests for post-write revocation.
    pub(crate) fn write_new_with(
        &self,
        bytes: &[u8],
        after: impl FnOnce(),
    ) -> io::Result<Metadata> {
        self.prepare_creation()?;
        let mut file = self.parent.create_new(&self.name)?;
        let result = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .and_then(|()| self.parent.sync());
        after();
        if let Err(error) = result
            .and_then(|()| self.check_current_policy())
            .and_then(|()| self.parent.verify_created(&self.name, &file))
        {
            let length = usize::try_from(file.metadata()?.len()).map_err(io::Error::other)?;
            let written = bytes
                .get(..length)
                .ok_or_else(|| io::Error::other("created file changed"))?;
            self.parent
                .remove_created_bytes(&self.name, &file, written)
                .map_err(|cleanup| {
                    io::Error::other(format!("{error}; rollback failed: {cleanup}"))
                })?;
            return Err(error);
        }
        file.metadata()
    }

    /// Publish a completed sibling state record without replacing any destination.
    ///
    /// # Errors
    /// Refuses denied source/destination paths, different parents and failed verified rollback.
    pub fn publish_from(&self, source: &AuthorizedPath<'_>, bytes: &[u8]) -> io::Result<()> {
        self.publish_from_with(source, bytes, || {})
    }

    /// Scheduling seam exercises post-publication rollback with live authority.
    pub(crate) fn publish_from_with(
        &self,
        source: &AuthorizedPath<'_>,
        bytes: &[u8],
        after: impl FnOnce(),
    ) -> io::Result<()> {
        self.publish_scheduled(
            source,
            bytes,
            PublicationChecks {
                after_source_open: || {},
                before_link: || {},
                after_link: after,
            },
        )
    }

    /// One shared publication effect, with a private scheduling seam around its policy checks.
    fn publish_scheduled(
        &self,
        source: &AuthorizedPath<'_>,
        bytes: &[u8],
        hooks: PublicationChecks<impl FnOnce(), impl FnOnce(), impl FnOnce()>,
    ) -> io::Result<()> {
        if source.access != Access::Read {
            return Err(io::Error::other("not a read capability"));
        }
        source.revalidate()?;
        if self.parent.canonical_path()? != source.parent.canonical_path()? {
            return Err(io::Error::other("publication requires one held parent"));
        }
        self.parent.publish_verified(
            &source.name,
            &self.name,
            bytes,
            PublicationChecks {
                after_source_open: || {
                    (hooks.after_source_open)();
                    source.check_current_policy()
                },
                before_link: || {
                    (hooks.before_link)();
                    self.prepare_creation()?;
                    source.check_current_policy()
                },
                after_link: || {
                    (hooks.after_link)();
                    self.check_current_policy()?;
                    source.check_current_policy()
                },
            },
        )
    }

    /// Rebind one source deny between the held read and the publication checks.
    #[cfg(test)]
    pub(crate) fn publish_before_for_test(
        &self,
        source: &AuthorizedPath<'_>,
        bytes: &[u8],
        before: impl FnOnce(),
        after: impl FnOnce(),
    ) -> io::Result<()> {
        self.publish_scheduled(
            source,
            bytes,
            PublicationChecks {
                after_source_open: || {},
                before_link: before,
                after_link: after,
            },
        )
    }

    /// Rebind immutable denies exactly after the source open, before any content read.
    #[cfg(test)]
    pub(crate) fn publish_after_open_for_test(
        &self,
        source: &AuthorizedPath<'_>,
        bytes: &[u8],
        after_open: impl FnOnce(),
        before_link: impl FnOnce(),
    ) -> io::Result<()> {
        self.publish_scheduled(
            source,
            bytes,
            PublicationChecks {
                after_source_open: after_open,
                before_link,
                after_link: || {},
            },
        )
    }

    /// Remove only owned bytes/identity through quarantine, restoring on policy refusal.
    ///
    /// # Errors
    /// Refuses read-only authority, changed policy, changed bytes/identity and failed restoration.
    /// As in C04, a same-user process can race the final check and unlink of the quarantine.
    pub fn remove_verified(&self, bytes: &[u8], identity: Option<(u64, u64)>) -> io::Result<()> {
        self.remove_verified_with(bytes, identity, || {})
    }

    /// Scheduling seam exercises policy restoration while the entry is quarantined.
    pub(crate) fn remove_verified_with(
        &self,
        bytes: &[u8],
        identity: Option<(u64, u64)>,
        after: impl FnOnce(),
    ) -> io::Result<()> {
        self.prepare_creation()?;
        self.parent
            .remove_verified_checked(&self.name, bytes, identity, || {
                after();
                self.check_current_policy()
            })
    }

    /// Rebind immutable denies after the pre-check, before native mkdir.
    #[cfg(test)]
    pub(crate) fn create_directory_between_for_test(
        &self,
        before: impl FnOnce(),
    ) -> io::Result<()> {
        self.create_directory_with(before, || {}).map(drop)
    }

    /// Scheduling seam for native mkdir rollback after an immutable deny binding changes.
    #[cfg(test)]
    pub(crate) fn create_directory_for_test(&self, after: impl FnOnce()) -> io::Result<()> {
        self.create_directory_with(|| {}, after).map(drop)
    }

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
        self.finish_read(self.parent.open_regular(&self.name)?)
    }

    /// Drop the opened handle on refusal rather than returning stale authority.
    fn finish_read(&self, file: File) -> io::Result<File> {
        self.check_current_policy()?;
        Ok(file)
    }

    /// Test-only scheduling hook between the lease check and regular-file open.
    #[cfg(test)]
    pub(crate) fn open_read_with(&self, after_check: impl FnOnce()) -> io::Result<File> {
        if self.access != Access::Read {
            return Err(io::Error::other("not a read capability"));
        }
        self.revalidate()?;
        after_check();
        self.finish_read(self.parent.open_regular(&self.name)?)
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
    /// A same-user process can replace the quarantine entry between the last check
    /// and unlinkat because POSIX has no unlink-if-same-file.
    /// Windows holds all ancestors without delete-sharing, blocking the rename.
    ///
    /// # Errors
    /// Refuses read-only capabilities, swaps, existing names and failed creation.
    #[cfg(test)]
    pub(crate) fn create_new(&self) -> io::Result<File> {
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

    /// Do not return a created handle if its parent or policy changed after the check.
    #[cfg(test)]
    fn finish_creation(&self, file: File) -> io::Result<File> {
        if let Err(error) = self.check_current_policy() {
            self.rollback_creation(file).map_err(|cleanup| {
                io::Error::other(format!(
                    "parent changed or policy changed: {error}; rollback failed: {cleanup}"
                ))
            })?;
            return Err(error);
        }
        Ok(file)
    }

    /// Delete only the empty object this operation created, never a replaced entry.
    #[cfg(test)]
    fn rollback_creation(&self, file: File) -> io::Result<()> {
        #[cfg(unix)]
        {
            let result = self.parent.remove_created_bytes(&self.name, &file, &[]);
            drop(file);
            result
        }
        #[cfg(windows)]
        {
            drop(file);
            self.parent.remove_verified(&self.name, &[], None)
        }
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
            && !self.trust.preferences.as_ref().is_some_and(|root| {
                self.supplied_base == *root
                    && self.relative == Path::new(".maestro/config.toml")
                    && parent.starts_with(root)
            })
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
