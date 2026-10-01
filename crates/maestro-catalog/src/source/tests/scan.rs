//! Aggregate snapshot trust-boundary neighbours, independent of content guards.

use super::support::{MemoryTree, check_by};
use crate::source::kinds::legacy as builtin;
use crate::{
    limits::Limits,
    source::{Cause, Entry, EntryKind, Registry, SourceTree, walk::walk},
};
use std::{cell::Cell, io};

/// A port that deliberately changes one source after its first bounded read.
struct ChangingTree {
    /// Its original valid catalog.
    original: MemoryTree,
    /// Reads of the chosen source.
    reads: Cell<usize>,
}

impl SourceTree for ChangingTree {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        self.original.list(directory)
    }
    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        if file == "mcp/maestro.toml" {
            let reads = self.reads.get();
            self.reads.set(reads + 1);
            if reads > 0 {
                return Ok(vec![b'x'; 1024]);
            }
        }
        self.original.read(file, max_bytes)
    }
}

#[test]
fn check_parses_only_the_aggregate_counted_snapshot_bytes() {
    let tree = ChangingTree {
        original: MemoryTree::valid(),
        reads: Cell::new(0),
    };
    let checked = check_by(&tree, &builtin().unwrap(), &Limits::PRODUCTION);
    assert!(
        checked.is_ok(),
        "must parse the counted first read: {checked:?}"
    );
    assert_eq!(tree.reads.get(), 1, "must not reopen a counted source");
}

/// A malformed adapter that can return duplicates/unsafe names or bypass a listing bound.
struct MalformedTree {
    /// Its root listing.
    entries: Vec<Entry>,
}

impl SourceTree for MalformedTree {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        assert!(directory.is_empty());
        Ok(self.entries.clone())
    }
    fn list_bounded(&self, directory: &str, _limit: usize) -> io::Result<Vec<Entry>> {
        self.list(directory)
    }
    fn read(&self, _file: &str, _max_bytes: u64) -> io::Result<Vec<u8>> {
        Ok(vec![])
    }
}

#[test]
fn snapshot_refuses_adapter_listings_past_the_remaining_bound() {
    let tree = MalformedTree {
        entries: vec![
            Entry {
                name: ".one".to_owned(),
                kind: EntryKind::File,
            },
            Entry {
                name: ".two".to_owned(),
                kind: EntryKind::File,
            },
        ],
    };
    let limits = Limits {
        archive_entries: 1,
        ..Limits::PRODUCTION
    };
    assert!(
        walk(&tree, &Registry::default(), &limits).is_err(),
        "adapter bound must refuse"
    );
    assert!(
        walk(&tree, &Registry::default(), &limits)
            .unwrap_err()
            .to_string()
            .contains("walk entry limit")
    );
}

#[test]
fn snapshot_refuses_unsafe_adapter_path_components() {
    let tree = MalformedTree {
        entries: vec![Entry {
            name: "../outside".to_owned(),
            kind: EntryKind::File,
        }],
    };
    let found = walk(&tree, &Registry::default(), &Limits::PRODUCTION).unwrap();
    assert_eq!(found.diagnostics.len(), 1);
    assert_eq!(
        found.diagnostics[0].message,
        "unsafe catalog path component"
    );
}

#[test]
fn snapshot_refuses_duplicate_source_paths_even_with_identical_bytes() {
    let entry = Entry {
        name: ".duplicate".to_owned(),
        kind: EntryKind::File,
    };
    let tree = MalformedTree {
        entries: vec![entry.clone(), entry],
    };
    let found = walk(&tree, &Registry::default(), &Limits::PRODUCTION).unwrap();
    assert_eq!(found.diagnostics.len(), 1);
    assert_eq!(found.diagnostics[0].message, "duplicate source path");
}

#[test]
fn source_tree_default_listing_is_bounded_without_scanner_masking() {
    let tree = MemoryTree::default().with("file.toml", "data");
    assert!(tree.list_bounded("", 1).is_ok());
    assert!(
        tree.list_bounded("", 0).is_err(),
        "default listing bound must refuse"
    );
    assert_eq!(
        tree.list_bounded("", 0).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn inert_source_read_and_listing_failures_cannot_be_skipped() {
    for tree in [
        MemoryTree::default()
            .with(".asset", "data")
            .with_unreadable(".asset"),
        MemoryTree::default()
            .with(".hidden/file", "data")
            .with_unreadable(".hidden"),
    ] {
        let found = walk(&tree, &Registry::default(), &Limits::PRODUCTION).unwrap();
        assert_eq!(found.diagnostics.len(), 1);
        assert_eq!(found.diagnostics[0].cause, Cause::Unreadable);
    }
}
