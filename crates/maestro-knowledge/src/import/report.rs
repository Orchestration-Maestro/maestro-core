//! What an import reports: how many entries it imported, found unchanged,
//! held and refused, and why it refused each one it did.

use maestro_kernel::artifact::Digest;
use serde::{Serialize, Serializer};

/// What an import of a collection did with the entries of its sources'
/// manifests: every line is counted once, as imported, unchanged, held or
/// refused, and each refusal is kept with its reason. It serializes as the
/// JSON of those counts and refusals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The collection imported.
    pub collection: String,
    /// The entries recorded as new revisions, not counting those held.
    pub imported: u64,
    /// The entries found recorded already, as they are: their revision, and
    /// its hold when they are held.
    pub unchanged: u64,
    /// The entries held back from indexing, their revisions new or recorded
    /// before, because another line of their manifest gives their
    /// `source_ref` other bytes.
    pub held: u64,
    /// The entries refused, each with its reason in
    /// [`refusals`](Report::refusals).
    pub refused: u64,
    /// Each refusal, in the order of the sources and their lines.
    pub refusals: Vec<Refusal>,
}

impl Report {
    /// The report of an import of `collection` that has counted nothing yet.
    pub(super) fn new(collection: &str) -> Self {
        Self {
            collection: collection.to_owned(),
            imported: 0,
            unchanged: 0,
            held: 0,
            refused: 0,
            refusals: Vec::new(),
        }
    }

    /// Counts what importing one entry did.
    pub(super) fn count(&mut self, imported: Imported) {
        let count = match imported {
            Imported::New => &mut self.imported,
            Imported::Unchanged => &mut self.unchanged,
            Imported::Held => &mut self.held,
        };
        *count += 1;
    }

    /// Counts the refusal of the line `line` of the manifest of `source`,
    /// and keeps its reason.
    pub(super) fn refuse(&mut self, source: &str, line: u64, reason: Reason) {
        self.refused += 1;
        self.refusals.push(Refusal {
            source: source.to_owned(),
            line,
            reason,
        });
    }
}

/// What importing an entry recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Imported {
    /// A new revision.
    New,
    /// Nothing: its revision, and its hold if it is held, were recorded
    /// before.
    Unchanged,
    /// Its revision's hold, with the revision itself in the same write when
    /// it is new.
    Held,
}

/// A line of a manifest the import refused, and why: the rest of the
/// manifest is imported all the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    /// The source whose manifest holds the line.
    pub source: String,
    /// The line's number in the manifest, from 1.
    pub line: u64,
    /// Why the import refused it, which the JSON gives as `reason`, beside
    /// the fields of that reason.
    #[serde(flatten)]
    pub reason: Reason,
}

/// Why the import refused a line of a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reason {
    /// The line is not a strict `maestro-corpus/1` entry, or not UTF-8.
    Malformed {
        /// What is wrong with it.
        message: String,
    },
    /// The file the line names cannot be read.
    Unreadable {
        /// The file, relative to the manifest's directory.
        path: String,
        /// What the operating system reported.
        message: String,
    },
    /// The file's bytes do not have the SHA-256 digest the line declares
    /// (docs/architecture/01 §2.1).
    DigestMismatch {
        /// The digest the line declares.
        #[serde(serialize_with = "hexadecimal")]
        declared: Digest,
        /// The digest of the file's bytes.
        #[serde(serialize_with = "hexadecimal")]
        found: Digest,
    },
    /// The file has the declared digest but not the declared size: the line
    /// contradicts itself.
    SizeMismatch {
        /// The size the line declares, in bytes.
        declared: u64,
        /// The file's size, in bytes.
        found: u64,
    },
    /// The file is not UTF-8, so not Markdown.
    NotUtf8,
    /// Canonicalization refused the document: its nesting is too deep, for
    /// one.
    Canonicalization {
        /// Canonicalization's reason.
        message: String,
    },
    /// The kernel holds the entry's bytes under another media type, or its
    /// document under another source: it keeps its record as it is.
    Conflict {
        /// The kernel's reason.
        message: String,
    },
}

/// `digest` as its 64 hexadecimal characters.
fn hexadecimal<S: Serializer>(digest: &Digest, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(digest.as_str())
}
