//! One bounded snapshot of every folder/file/link in the source tree. Check
//! and publish cover the same tree, so walk entries/bytes use the archive bounds;
//! resource counts retain their separate bound. No package/mount resets counters.

use super::{
    discovered::refusal,
    placements::{join, safe},
    tree::{Entry, EntryKind, SourceTree},
    types::{Diagnostic, Refusal},
};
use crate::limits::Limits;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

/// The result of the only original read of a source file.
enum CachedFile {
    /// Original bounded bytes.
    Bytes(Vec<u8>),
    /// The original error, reproduced without reopening the source.
    Unreadable(io::ErrorKind, String),
}

/// Bounded listings, reused by discovery instead of walking the filesystem twice.
pub(super) struct Snapshot {
    /// Counted original bytes or their original read failure, never reopened.
    files: BTreeMap<String, CachedFile>,
    /// Listings by relative directory; includes empty directories.
    pub(super) directories: BTreeMap<String, Vec<Entry>>,
    /// Unreadable, unsafe or linked entries.
    pub(super) diagnostics: Vec<Diagnostic>,
}

impl SourceTree for Snapshot {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        self.directories
            .get(directory)
            .cloned()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        match self.files.get(file) {
            Some(CachedFile::Bytes(bytes)) => {
                let length = usize::try_from(max_bytes.saturating_add(1)).unwrap_or(usize::MAX);
                Ok(bytes.iter().take(length).copied().collect())
            }
            Some(CachedFile::Unreadable(kind, message)) => {
                Err(io::Error::new(*kind, message.clone()))
            }
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }
}

/// Snapshot the whole tree under one aggregate budget, before content parsing.
pub(super) fn scan(tree: &dyn SourceTree, limits: &Limits) -> Result<Snapshot, Refusal> {
    let mut snapshot = Snapshot {
        files: BTreeMap::new(),
        directories: BTreeMap::new(),
        diagnostics: Vec::new(),
    };
    let mut pending = vec![(String::new(), 0)];
    let mut entries = 0;
    let mut bytes = 0;
    while let Some((directory, depth)) = pending.pop() {
        let listed = listing(
            tree,
            &directory,
            limits.archive_entries - entries,
            &mut snapshot.diagnostics,
            limits.archive_entries,
        )?;
        entries += listed.len();
        let mut names = BTreeSet::new();
        for entry in &listed {
            let path = join(&directory, &entry.name);
            if !names.insert(&entry.name) {
                snapshot
                    .diagnostics
                    .push(Diagnostic::new(&path, "", "duplicate source path"));
                continue;
            }
            if entry.name.is_empty() || entry.name.contains('/') || !safe(&entry.name, false) {
                snapshot.diagnostics.push(Diagnostic::new(
                    &path,
                    "",
                    "unsafe catalog path component",
                ));
                continue;
            }
            if depth >= limits.manifest_depth {
                return Err(refusal(format!(
                    "more than {} walk depth levels",
                    limits.manifest_depth
                )));
            }
            match entry.kind {
                EntryKind::Directory => pending.push((path, depth + 1)),
                EntryKind::Unsupported => snapshot.diagnostics.push(Diagnostic::new(
                    path,
                    "",
                    "links and special files are not read",
                )),
                EntryKind::File => {
                    bytes += count_file(
                        tree,
                        &path,
                        limits,
                        limits.archive_total_bytes - bytes,
                        &mut snapshot,
                    )?;
                }
            }
        }
        snapshot.directories.insert(directory, listed);
    }
    Ok(snapshot)
}

/// Count one bounded source/asset read against the remaining aggregate bytes.
fn count_file(
    tree: &dyn SourceTree,
    path: &str,
    limits: &Limits,
    remaining: u64,
    snapshot: &mut Snapshot,
) -> Result<u64, Refusal> {
    let contents = match tree.read(path, remaining.min(limits.source_file_bytes)) {
        Ok(contents) => contents,
        Err(error) => {
            snapshot.diagnostics.push(Diagnostic::unreadable(
                path,
                format!("cannot read: {error}"),
            ));
            snapshot.files.insert(
                path.to_owned(),
                CachedFile::Unreadable(error.kind(), error.to_string()),
            );
            return Ok(0);
        }
    };
    let length = u64::try_from(contents.len()).unwrap_or(u64::MAX);
    if length > remaining {
        return Err(refusal(format!(
            "more than {} source bytes",
            limits.archive_total_bytes
        )));
    }
    if length > limits.source_file_bytes {
        snapshot.diagnostics.push(Diagnostic::new(
            path,
            "",
            format!("larger than {} bytes", limits.source_file_bytes),
        ));
    }
    snapshot
        .files
        .insert(path.to_owned(), CachedFile::Bytes(contents));
    Ok(length)
}

/// List under the remaining aggregate bound; unreadable subtrees are diagnostic.
fn listing(
    tree: &dyn SourceTree,
    directory: &str,
    remaining: usize,
    diagnostics: &mut Vec<Diagnostic>,
    total: usize,
) -> Result<Vec<Entry>, Refusal> {
    match tree.list_bounded(directory, remaining) {
        Ok(entries) if entries.len() <= remaining => Ok(entries),
        Ok(_) => Err(refusal("walk entry limit exceeded by adapter".to_owned())),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            Err(refusal(format!("more than {total} walk entries: {error}")))
        }
        Err(error) if directory.is_empty() => Err(Refusal {
            diagnostics: vec![Diagnostic::unreadable(
                "",
                format!("cannot list the catalog directory: {error}"),
            )],
        }),
        Err(error) => {
            diagnostics.push(Diagnostic::unreadable(
                directory,
                format!("cannot list: {error}"),
            ));
            Ok(Vec::new())
        }
    }
}
