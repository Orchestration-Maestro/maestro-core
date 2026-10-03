//! Owned Windows controls and receipts retain the shared no-follow ancestry.
use crate::{
    windows::{Directory, hold, hold_directory, refuse_reparse_point},
    windows_security::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ_WRITE,
    },
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind},
    ops::BitOr as _,
    os::windows::fs::OpenOptionsExt as _,
    path::Path,
};
use winapi_util::file::information;

impl Directory {
    /// Reopen the root without following links and compare it with the retained identity.
    pub(crate) fn validate_owned(&self, path: &Path) -> io::Result<()> {
        let named = hold_directory(path)?;
        let held = self
            .held
            .last()
            .ok_or_else(|| io::Error::other("owned root has no retained directory"))?;
        validate_identity(held, &named)
    }

    /// Open or create a control file. ACL privacy relies on inheritance from the
    /// user-private data directory, not a separate check (no approved ACL API).
    pub(crate) fn open_control(&self, name: &str, create: bool) -> io::Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(create)
            .share_mode(FILE_SHARE_READ_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.bitor(FILE_FLAG_OPEN_REPARSE_POINT))
            .open(self.path.join(name))?;
        validate_control_file(&file)?;
        if create {
            file.sync_all()?;
        }
        Ok(file)
    }

    /// Recheck the held file and its name inside the retained graph root before locking.
    pub(crate) fn validate_control(&self, name: &str, file: &File) -> io::Result<()> {
        validate_control_file(file)?;
        let named = self.open_control(name, false)?;
        validate_identity(file, &named)
    }

    /// Retain the no-follow leaf against deletion/replacement; inspect its held link count.
    pub(crate) fn open_receipt_file(&self, name: &str) -> io::Result<File> {
        let file = hold(
            &self.path.join(name),
            FILE_FLAG_BACKUP_SEMANTICS.bitor(FILE_FLAG_OPEN_REPARSE_POINT),
        )?;
        refuse_reparse_point(&file)?;
        if !file.metadata()?.is_file() || information(&file)?.number_of_links() != 1 {
            return Err(io::Error::other(
                "receipt file must be regular with one link",
            ));
        }
        Ok(file)
    }

    /// Safe Rust exposes neither held-leaf deletion nor directory durability here.
    /// Validate identity but refuse rather than release the leaf for a path-only fallback.
    pub(crate) fn remove_receipt_file(
        &self,
        name: &str,
        expected: Option<&File>,
    ) -> io::Result<bool> {
        match self.open_receipt_file(name) {
            Ok(named) => {
                let held = expected
                    .ok_or_else(|| io::Error::other("receipt file appeared after lookup"))?;
                validate_identity(held, &named)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Err(io::Error::new(
            ErrorKind::Unsupported,
            "anchored receipt removal and directory sync are unsupported on Windows",
        ))
    }

    /// Reserve one new directory while retaining all ancestors against relocation.
    pub(crate) fn reserve_child(&self, name: &str) -> io::Result<Self> {
        let path = self.path.join(name);
        fs::create_dir(&path)?;
        Self::open_resolved(&path, false)
    }

    /// Native Windows writes are deliberately unsupported; there is no path-only fallback.
    pub(crate) fn install_from(&self, _staging: &Self, _name: &str) -> io::Result<()> {
        Err(io::Error::new(
            ErrorKind::Unsupported,
            format!(
                "Windows native publication is unsupported under {}",
                self.path.display()
            ),
        ))
    }

    /// Check a receipt child through its held file identity, refusing hard-link aliases.
    pub(crate) fn check_regular(&self, name: &str) -> io::Result<()> {
        let file = self.open_regular(name)?;
        if information(&file)?.number_of_links() != 1 {
            return Err(io::Error::other("receipt file must have one link"));
        }
        Ok(())
    }
}

/// Refuse a guard that is a reparse point, nonregular file or hard-link alias.
fn validate_control_file(file: &File) -> io::Result<()> {
    refuse_reparse_point(file)?;
    if !file.metadata()?.is_file() || information(file)?.number_of_links() != 1 {
        return Err(io::Error::other(
            "control file must be regular with one link",
        ));
    }
    Ok(())
}

/// Bind a safely reopened name to the retained handle's volume and file identity.
fn validate_identity(held: &File, named: &File) -> io::Result<()> {
    let held = information(held)?;
    let current = information(named)?;
    if (held.volume_serial_number(), held.file_index())
        != (current.volume_serial_number(), current.file_index())
    {
        return Err(io::Error::other("held filesystem identity was replaced"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Directory;
    use maestro_test_scratch::scratch_directory;
    use std::{fs, io};

    #[test]
    fn filesystem_windows_controls_allow_read_write_sharing() {
        use std::io::Write;
        let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
        let root = Directory::open_resolved(&scratch, false).unwrap();
        let mut first = root.open_control("guard", true).unwrap();
        first.write_all(b"first").unwrap();
        let reopened = root.open_control("guard", false);
        assert!(reopened.is_ok(), "read/write sharing refused: {reopened:?}");
        let mut second = reopened.unwrap();
        second.write_all(b"second").unwrap();
        assert_eq!(fs::read(scratch.join("guard")).unwrap(), b"second");
        drop((second, first, root));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn filesystem_windows_open_flags_inspect_dangling_reparse_leaves() {
        use std::process::Command;
        let scratch = scratch_directory().unwrap();
        let root = Directory::open_resolved(&fs::canonicalize(&scratch).unwrap(), false).unwrap();
        let linked = scratch.join("linked");
        assert!(
            Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&linked)
                .arg(scratch.join("absent"))
                .status()
                .unwrap()
                .success()
        );
        let control = root.open_control("linked", false).unwrap_err();
        let receipt = root.open_receipt_file("linked").unwrap_err();
        assert_eq!(control.to_string(), "path names a link or reparse point");
        assert_eq!(receipt.to_string(), "path names a link or reparse point");
        assert!(!scratch.join("absent").exists());
        drop(root);
        fs::remove_dir(linked).unwrap();
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn filesystem_windows_removal_distinguishes_absence_from_unsafe_leaf() {
        let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
        let root = Directory::open_resolved(&scratch, false).unwrap();
        assert_eq!(
            root.remove_receipt_file("absent", None).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        fs::create_dir(scratch.join("directory")).unwrap();
        let refused = root.remove_receipt_file("directory", None).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::Other);
        assert_eq!(
            refused.to_string(),
            "receipt file must be regular with one link"
        );
        assert!(scratch.join("directory").is_dir());
        drop(root);
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn filesystem_windows_removal_refuses_mismatched_held_identity() {
        let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
        let root = Directory::open_resolved(&scratch, false).unwrap();
        fs::write(scratch.join("receipt"), b"receipt sentinel").unwrap();
        fs::write(scratch.join("other"), b"other sentinel").unwrap();
        let matching = root.open_receipt_file("receipt").unwrap();
        let mismatched = root.open_receipt_file("other").unwrap();
        let refused = root
            .remove_receipt_file("receipt", Some(&mismatched))
            .unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::Other);
        assert_eq!(refused.to_string(), "held filesystem identity was replaced");
        assert_eq!(
            root.remove_receipt_file("receipt", Some(&matching))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            fs::read(scratch.join("receipt")).unwrap(),
            b"receipt sentinel"
        );
        assert_eq!(fs::read(scratch.join("other")).unwrap(), b"other sentinel");
        drop((mismatched, matching, root));
        fs::remove_dir_all(scratch).unwrap();
    }
}
