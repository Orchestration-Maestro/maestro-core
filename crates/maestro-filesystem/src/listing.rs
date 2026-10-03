//! Shared bounded listing records; platform adapters classify without following links.

use std::{ffi::OsString, io};

/// What a held directory's entry names, without following links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link or Windows reparse point.
    Link,
    /// A device, pipe or other special entry.
    Other,
}

/// One entry classified relative to an already-held directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its single-component name, retaining non-UTF-8 names.
    pub name: OsString,
    /// Its no-follow classification.
    pub kind: EntryKind,
}

/// Retains at most `limit` entries, refusing the first extra entry.
pub(crate) fn push(entries: &mut Vec<Entry>, entry: Entry, limit: usize) -> io::Result<()> {
    if entries.len() >= limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "directory entry limit exceeded",
        ));
    }
    entries.push(entry);
    Ok(())
}

/// Classify no-follow Windows handle metadata; reparse points always win.
#[cfg(any(windows, test))]
pub(crate) fn windows_kind(attributes: u32, directory: bool, file: bool) -> EntryKind {
    if attributes & 0x0000_0400 != 0 {
        EntryKind::Link
    } else if directory {
        EntryKind::Directory
    } else if file {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}
