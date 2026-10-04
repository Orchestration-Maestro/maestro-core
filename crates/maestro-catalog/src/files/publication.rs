//! The one C04 complete-record writer, shared by exclusive creates and replacement journals.
use super::effects;
use crate::policy::workspace::{Access, CheckedTrust};
use std::{
    io::{self, ErrorKind},
    path::Path,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Unique process-local suffixes for unpublished state-record files.
static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

/// Write a complete state record privately, then publish it atomically without replacement.
pub(super) fn write_new(
    root: &Path,
    (name, bytes): (&str, &[u8]),
    tear: bool,
    trust: &CheckedTrust<'_>,
) -> io::Result<()> {
    let temporary = loop {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let (parent, leaf) = name.rsplit_once('/').unwrap_or(("", name));
        let prefix = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}/")
        };
        let temporary = format!("{prefix}.{leaf}.tmp-{}-{sequence}", process::id());
        let written = if tear {
            bytes.get(..bytes.len() / 2).unwrap_or_default()
        } else {
            bytes
        };
        match effects::write(root, &temporary, written, trust) {
            Ok(_) => break temporary,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    };
    if tear {
        return Err(io::Error::other("injected torn state-record write"));
    }
    let source = trust.authorize(root, Path::new(&temporary), Access::Read)?;
    trust
        .authorize(root, Path::new(name), Access::Write)?
        .publish_from(&source, bytes)?;
    effects::remove(root, &temporary, bytes, None, trust)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        files::tests::support::with_trust,
        policy::workspace::{TrustBoundaries, WorkspaceTrust},
    };
    use maestro_test_scratch::scratch_directory;
    use std::{cell::Cell, fs, path::PathBuf};

    #[test]
    fn files_publication_torn_bytes_and_existing_temporary_names() {
        let root = scratch_directory().unwrap();
        with_trust(&root, |trust| {
            assert!(write_new(&root, ("record", b"abcdef"), true, trust).is_err());
            assert!(!root.join("record").exists());
            let torn = fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
            assert_eq!(fs::read(&torn).unwrap(), b"abc");
            fs::remove_file(torn).unwrap();
            let next = NEXT_TEMP.load(Ordering::Relaxed);
            let collision = root.join(format!(".record.tmp-{}-{next}", process::id()));
            fs::write(&collision, b"user").unwrap();
            write_new(&root, ("record", b"complete"), false, trust).unwrap();
            assert_eq!(fs::read(root.join("record")).unwrap(), b"complete");
            assert_eq!(fs::read(collision).unwrap(), b"user");
        });
        fs::remove_dir_all(root).unwrap();
    }

    /// Refuse just the first effect so retrying a non-collision would wrongly succeed.
    struct OnceDenied {
        root: PathBuf,
        denied: Cell<bool>,
    }
    impl WorkspaceTrust for OnceDenied {
        fn containing_root(&self, path: &Path) -> Option<PathBuf> {
            if self.denied.replace(false) {
                return None;
            }
            path.starts_with(&self.root).then(|| self.root.clone())
        }
    }
    #[test]
    fn files_publication_does_not_retry_policy_errors() {
        let root = scratch_directory().unwrap().canonicalize().unwrap();
        let authority = OnceDenied {
            root: root.clone(),
            denied: Cell::new(true),
        };
        let boundaries = TrustBoundaries::new(root.parent().unwrap(), &[]).unwrap();
        let trust = CheckedTrust::new(&authority, &boundaries);
        assert!(write_new(&root, ("record", b"complete"), false, &trust).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
