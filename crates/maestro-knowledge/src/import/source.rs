//! Importing the manifest of one source: a first pass finds the
//! `source_ref`s its lines give different digests, keeping one digest per
//! `source_ref` and no line, then each line is imported in turn, one line and
//! one document at a time.

use super::{
    corpus::Corpus,
    entry::{self, NotImported, Target},
    error::Error,
    report::{Imported, Reason, Report},
};
use crate::corpus::{self, Entry};
use maestro_kernel::artifact::Digest;
use std::{
    collections::{BTreeSet, HashMap},
    io::{self, BufRead},
    str,
};

/// Imports each line of the manifest of `corpus` into `target`'s source,
/// and counts what each did in `report`: a line refused is kept there with
/// its number and reason, and the next line is imported all the same.
///
/// # Errors
///
/// [`Error::Manifest`] when the manifest cannot be read, and the kernel's
/// failures, which stop the import where it is.
pub(super) fn import_source(
    target: &Target<'_>,
    corpus: &impl Corpus,
    report: &mut Report,
) -> Result<(), Error> {
    let shared = shared_source_refs(target, corpus)?;
    let mut lines = Lines::new(
        corpus
            .manifest()
            .map_err(|error| unreadable(target, error))?,
    );
    while let Some(line) = lines.next().map_err(|error| unreadable(target, error))? {
        let imported = match line.text {
            Ok(text) => import_line(target, corpus, text, &shared),
            Err(reason) => Err(NotImported::Refused(reason)),
        };
        match imported {
            Ok(imported) => report.count(imported),
            Err(NotImported::Refused(reason)) => report.refuse(target.source, line.number, reason),
            Err(NotImported::Failed(error)) => return Err(error),
        }
    }
    Ok(())
}

/// Imports the line `text`, refused as malformed unless it is a strict
/// `maestro-corpus/1` entry.
fn import_line(
    target: &Target<'_>,
    corpus: &impl Corpus,
    text: &str,
    shared: &BTreeSet<String>,
) -> Result<Imported, NotImported> {
    let entry: Entry = text
        .parse()
        .map_err(|error: corpus::Error| Reason::Malformed {
            message: error.to_string(),
        })?;
    entry::import(target, corpus, &entry, shared.contains(&entry.source_ref))
}

/// The `source_ref`s that lines of the manifest of `corpus` give different
/// digests. A line that is no entry names none; the second pass refuses it.
fn shared_source_refs(
    target: &Target<'_>,
    corpus: &impl Corpus,
) -> Result<BTreeSet<String>, Error> {
    let mut digests: HashMap<String, Digest> = HashMap::new();
    let mut shared = BTreeSet::new();
    let mut lines = Lines::new(
        corpus
            .manifest()
            .map_err(|error| unreadable(target, error))?,
    );
    while let Some(line) = lines.next().map_err(|error| unreadable(target, error))? {
        let Some(entry) = line.text.ok().and_then(|text| text.parse::<Entry>().ok()) else {
            continue;
        };
        match digests.get(&entry.source_ref) {
            Some(first) if *first != entry.sha256 => {
                shared.insert(entry.source_ref);
            }
            Some(_) => {}
            None => {
                digests.insert(entry.source_ref, entry.sha256);
            }
        }
    }
    Ok(shared)
}

/// The failure to read the manifest of `target`'s source.
fn unreadable(target: &Target<'_>, error: io::Error) -> Error {
    Error::Manifest {
        source_id: target.source.to_owned(),
        error,
    }
}

/// A manifest read one line at a time, numbered from 1, into one buffer.
#[derive(Debug)]
pub(super) struct Lines<R> {
    /// The manifest.
    reader: R,
    /// The line read last, with its newline.
    buffer: Vec<u8>,
    /// Its number.
    number: u64,
}

/// A line of a manifest.
// No `Default`, so that a `next` that never ends cannot compile.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Line<'buffer> {
    /// Its number, from 1.
    pub(super) number: u64,
    /// Its text without its newline, or the refusal of a line that is not
    /// UTF-8.
    pub(super) text: Result<&'buffer str, Reason>,
}

impl<R: BufRead> Lines<R> {
    /// The lines of `reader`, from its first.
    pub(super) fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            number: 0,
        }
    }

    /// The next line, none after the last: the text after the last newline
    /// is a line when it is not empty.
    pub(super) fn next(&mut self) -> io::Result<Option<Line<'_>>> {
        self.buffer.clear();
        if self.reader.read_until(b'\n', &mut self.buffer)? == 0 {
            return Ok(None);
        }
        self.number += 1;
        let bytes = self.buffer.strip_suffix(b"\n").unwrap_or(&self.buffer);
        let text = str::from_utf8(bytes).map_err(|error| Reason::Malformed {
            message: format!("the line is not UTF-8: {error}"),
        });
        Ok(Some(Line {
            number: self.number,
            text,
        }))
    }
}
