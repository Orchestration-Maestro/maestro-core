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
