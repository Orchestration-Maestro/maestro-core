//! Unix created objects retain identity through hardening, publication and rollback.
use crate::{
    root::leaf_name,
    unix::{Directory, NEXT_QUARANTINE},
};
use rustix::fs::{AtFlags, Mode, RenameFlags, fstat, mkdirat, renameat_with, statat, unlinkat};
use std::{fs::File, io, process, sync::atomic::Ordering};

impl Directory {
    /// Exclusively create and hold one child directory without following links.
    ///
    /// # Errors
    /// Refuses existing names and failed creation, opens or directory syncs.
    pub fn create_child(&self, name: &str) -> io::Result<Self> {
        leaf_name(name)?;
        mkdirat(&self.0, name, Mode::RWXU)?;
        let child = self.child(name)?;
        self.sync()?;
        Ok(child)
    }

    /// Retain a no-follow child only when it is still the object exclusively created here.
    ///
    /// # Errors
    /// Refuses replacements and failed opens or identity queries.
    pub fn harden_created_child(&self, name: &str, created: &Self) -> io::Result<Self> {
        let child = self.child(name)?;
        self.verify_created(name, &created.0)?;
        Ok(child)
    }

    /// Remove only this still-empty created directory, comparing its held identity.
    /// A same-user quarantine swap can at worst remove another empty directory.
    ///
    /// # Errors
    /// Refuses replacements, non-empty directories and failed non-replacing restoration.
    pub fn remove_created_child(&self, name: &str, created: &Self) -> io::Result<()> {
        leaf_name(name)?;
        let quarantine = format!(
            ".{name}.maestro-directory-{}-{}",
            process::id(),
            NEXT_QUARANTINE.fetch_add(1, Ordering::Relaxed)
        );
        renameat_with(&self.0, name, &self.0, &quarantine, RenameFlags::NOREPLACE)?;
        let result = (|| {
            self.verify_created(&quarantine, &created.0)?;
            Ok(unlinkat(&self.0, quarantine.as_str(), AtFlags::REMOVEDIR)?)
        })();
        if let Err(error) = result {
            renameat_with(&self.0, &quarantine, &self.0, name, RenameFlags::NOREPLACE)?;
            return Err(error);
        }
        self.sync()
    }

    /// Check a named leaf against a still-held created file or directory without following links.
    ///
    /// # Errors
    /// Refuses replaced names and failed no-follow metadata queries.
    pub fn verify_created(&self, name: &str, created: &File) -> io::Result<()> {
        leaf_name(name)?;
        let held = fstat(created)?;
        let named = statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW)?;
        if named.st_dev != held.st_dev || named.st_ino != held.st_ino {
            return Err(io::Error::other(
                "created object changed; replacement not deleted",
            ));
        }
        Ok(())
    }
    /// Remove an empty created file using its still-held identity, without reopening it.
    /// Use a regular-file handle returned by `create_new` and keep it open until removal
    /// completes to prevent inode reuse.
    ///
    /// # Errors
    /// Refuses non-empty entries, replacements and failed cleanup.
    pub fn remove_created(&self, name: &str, created: &File) -> io::Result<()> {
        self.remove_created_bytes(name, created, &[])
    }

    /// Roll back exactly the bytes written to a still-held created file.
    ///
    /// # Errors
    /// Refuses replacements, changed bytes and failed restoration or removal.
    pub fn remove_created_bytes(&self, name: &str, created: &File, bytes: &[u8]) -> io::Result<()> {
        self.remove_created_inner(name, created, bytes, || {})
    }

    /// Verify the created identity after the quarantine scheduling hook.
    #[cfg(test)]
    pub(crate) fn remove_created_with(
        &self,
        name: &str,
        created: &File,
        after: impl FnOnce(),
    ) -> io::Result<()> {
        self.remove_created_inner(name, created, &[], after)
    }

    /// One identity comparison protects empty and written-file rollback through quarantine.
    fn remove_created_inner(
        &self,
        name: &str,
        created: &File,
        expected: &[u8],
        after: impl FnOnce(),
    ) -> io::Result<()> {
        leaf_name(name)?;
        self.remove_quarantined(name, after, |quarantine| {
            self.verify_created(quarantine, created)?;
            let matches = if expected.is_empty() {
                statat(&self.0, quarantine, AtFlags::SYMLINK_NOFOLLOW)?.st_size == 0
            } else {
                self.read_regular(quarantine)? == expected
            };
            if !matches {
                return Err(io::Error::other("created file bytes changed"));
            }
            Ok(())
        })
    }
}
