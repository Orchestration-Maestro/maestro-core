//! Verified replacement of one regular leaf through a retained parent.
#[cfg(windows)]
use crate::windows_security::same_file;
use crate::{Directory, root::leaf_name};
use std::{
    fs::File,
    io::{self, Read as _, Seek as _, Write as _},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Process-local private sibling names, always created exclusively.
static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

impl Directory {
    /// Replace a regular leaf only after verifying its held identity and expected bytes.
    /// The replacement is synced before an atomic rename and retains the original permissions.
    /// `before_swap` rechecks authority after preparation; it is never supplied by file content.
    /// A same-user writer can race the final comparison and rename: this is not a true CAS.
    /// Windows retains non-renamable parents and uses the standard library's replace-existing
    /// rename; its directory sync has the same platform limitation as other publications.
    ///
    /// # Errors
    /// Refuses unsafe names, links, special files, changed bytes/identity, policy and I/O failures.
    pub fn replace_verified(
        &self,
        name: &str,
        expected: &[u8],
        replacement: &[u8],
        before_swap: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        self.replace_verified_with(name, (expected, replacement), before_swap, || self.sync())
    }

    /// The mandatory post-publication step is real directory sync except in private failure tests.
    pub(crate) fn replace_verified_with(
        &self,
        name: &str,
        (expected, replacement): (&[u8], &[u8]),
        before_swap: impl FnOnce() -> io::Result<()>,
        after_publication: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        leaf_name(name)?;
        let mut original = self.open_regular(name)?;
        compare_bytes(&mut original, expected)?;
        let (temporary, mut staged) = self.replacement_temp()?;
        let mut contents_written = false;
        let result = (|| {
            #[cfg(windows)]
            self.retain_replacement_security(&temporary, &staged, &original)?;
            staged.write_all(replacement)?;
            contents_written = true;
            staged.set_permissions(original.metadata()?.permissions())?;
            staged.sync_all()?;
            before_swap()?;
            self.verify_created(name, &original)?;
            compare_bytes(&mut original, expected)?;
            let mut prepared = self.open_regular(&temporary)?;
            if !same_file(&staged, &prepared)? {
                return Err(io::Error::other("prepared replacement identity changed"));
            }
            self.verify_created(&temporary, &prepared)?;
            compare_bytes(&mut prepared, replacement)?;
            self.rename_replacement(&temporary, name)?;
            after_publication()
        })();
        if result.is_err() {
            // After rename the temporary is absent; never roll back the published new file.
            match self.open_regular(&temporary) {
                Ok(_) => self.remove_created_bytes(
                    &temporary,
                    &staged,
                    if contents_written { replacement } else { &[] },
                )?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        result
    }

    /// Create one exclusive sibling through the same held directory as the target.
    fn replacement_temp(&self) -> io::Result<(String, File)> {
        loop {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let name = format!(".maestro-replace-{}-{sequence}", process::id());
            match self.create_new(&name) {
                Ok(file) => return Ok((name, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }
}

/// Compare both retained Unix handles, not two potentially different named lookups.
#[cfg(unix)]
fn same_file(left: &File, right: &File) -> io::Result<bool> {
    use rustix::fs::fstat;
    let left = fstat(left)?;
    let right = fstat(right)?;
    Ok((left.st_dev, left.st_ino) == (right.st_dev, right.st_ino))
}

/// Compare through a retained regular handle, bounding the read by the previewed length.
fn compare_bytes(file: &mut File, expected: &[u8]) -> io::Result<()> {
    file.rewind()?;
    let mut bytes = Vec::new();
    file.take(expected.len() as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes != expected {
        return Err(io::Error::other(
            "file changed since preview; run preview again",
        ));
    }
    Ok(())
}
