//! Importing the manifest of one source: a first pass keeps one digest or
//! conflict marker per distinct `source_ref` and no line, then each line is
//! imported in turn, one line and one document at a time, the caller's
//! observer shown the report every hundred lines and after the last.

use super::{
    corpus::Corpus,
    entry,
    error::Error,
    ingest::{NotImported, Target},
    report::{Imported, Reason, Report},
};
use crate::corpus::{self, Entry};
use maestro_kernel::artifact::Digest;
use std::{
    collections::HashMap,
    io::{self, BufRead},
    ops::ControlFlow,
    str,
};

/// How many lines of a manifest an import reads between two looks of its
/// observer.
const BATCH: u64 = 100;

/// Imports each line of the manifest of `corpus` into `target`'s source,
/// and counts what each did in `report`: a line refused is kept there with
/// its number and reason, and the next line is imported all the same.
/// `observer` sees `report` after every [`BATCH`]th line and after the last
/// line, once each.
///
/// # Errors
///
/// [`Error::Manifest`] when the manifest cannot be read, the kernel's
/// failures, which stop the import where it is, and [`Error::Stopped`] when
/// `observer` breaks.
pub(super) fn import_source(
    target: &Target<'_>,
    corpus: &impl Corpus,
    report: &mut Report,
    observer: &mut impl FnMut(&Report) -> ControlFlow<()>,
) -> Result<(), Error> {
    let shared = shared_source_refs(target, corpus)?;
    let mut lines = Lines::new(
        corpus
            .manifest()
            .map_err(|error| unreadable(target, error))?,
    );
    while let Some(line) = lines.next().map_err(|error| unreadable(target, error))? {
        let number = line.number;
        let imported = match line.text {
            Ok(text) => import_line(target, corpus, text, &shared),
            Err(reason) => Err(NotImported::Refused(reason)),
        };
        match imported {
            Ok(imported) => report.count(imported),
            Err(NotImported::Refused(reason)) => report.refuse(target.source, number, reason),
            Err(NotImported::Failed(error)) => return Err(error),
        }
        if number % BATCH == 0 {
            observe(observer, report)?;
        }
    }
    if lines.number % BATCH == 0 {
        return Ok(());
    }
    observe(observer, report)
}

/// Shows `report` to `observer`.
///
/// # Errors
///
/// [`Error::Stopped`] when `observer` breaks.
fn observe(
    observer: &mut impl FnMut(&Report) -> ControlFlow<()>,
    report: &Report,
) -> Result<(), Error> {
    match observer(report) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(()) => Err(Error::Stopped),
    }
}

/// Imports the line `text`, refused as malformed unless it is a strict
/// `maestro-corpus/1` entry.
fn import_line(
    target: &Target<'_>,
    corpus: &impl Corpus,
    text: &str,
    references: &HashMap<String, Option<Digest>>,
) -> Result<Imported, NotImported> {
    let entry: Entry = text
        .parse()
        .map_err(|error: corpus::Error| Reason::Malformed {
            message: error.to_string(),
        })?;
    let shared = references
        .get(&entry.source_ref)
        .is_some_and(Option::is_none);
    entry::import(target, corpus, &entry, shared)
}

/// One entry per distinct `source_ref`: its digest, or `None` when lines
/// disagree. A line that is no entry names none; the second pass refuses it.
fn shared_source_refs(
    target: &Target<'_>,
    corpus: &impl Corpus,
) -> Result<HashMap<String, Option<Digest>>, Error> {
    let mut digests: HashMap<String, Option<Digest>> = HashMap::new();
    let mut lines = Lines::new(
        corpus
            .manifest()
            .map_err(|error| unreadable(target, error))?,
    );
    while let Some(line) = lines.next().map_err(|error| unreadable(target, error))? {
        let Some(entry) = line.text.ok().and_then(|text| text.parse::<Entry>().ok()) else {
            continue;
        };
        match digests.get_mut(&entry.source_ref) {
            Some(first) if first.as_ref().is_some_and(|first| first != &entry.sha256) => {
                *first = None;
            }
            Some(_) => {}
            None => {
                digests.insert(entry.source_ref, Some(entry.sha256));
            }
        }
    }
    Ok(digests)
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

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_kernel::{
        scope::{Right, ScopeSet},
        store::Database,
    };
    use maestro_test_scratch::scratch_directory;
    use std::{fs, io::Cursor, path::PathBuf};

    /// Distinct synthetic references, each repeated once in the manifest.
    const REFERENCES: usize = 2_048;

    /// Temporary kernel storage for the scale test.
    #[derive(Debug)]
    struct Scratch(PathBuf);

    impl Scratch {
        /// Creates a fresh test directory.
        fn new() -> Self {
            Self(scratch_directory().unwrap())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    /// A synthetic manifest whose document reads are deliberately refused.
    #[derive(Debug)]
    struct Manifests(Vec<u8>);

    impl Corpus for Manifests {
        fn manifest(&self) -> io::Result<impl BufRead> {
            Ok(Cursor::new(self.0.as_slice()))
        }

        fn document(&self, _path: &crate::RelativePath) -> io::Result<Vec<u8>> {
            Err(io::Error::from(io::ErrorKind::Unsupported))
        }
    }

    /// Each identity gets one bookkeeping slot and each refused line one record.
    #[test]
    fn bookkeeping_scales_with_distinct_references_and_refusals() {
        let mut manifest = Vec::new();
        for index in 0..REFERENCES {
            let body = format!("synthetic body {index}");
            let line = serde_json::json!({
                "schema": "maestro-corpus/1",
                "path": format!("pages/{index}.md"),
                "sha256": Digest::of(body.as_bytes()).as_str(),
                "bytes": body.len(),
                "source_ref": format!("https://example.org/pages/{index}"),
                "title": format!("Synthetic page {index}"),
                "source_kind": "guide",
            })
            .to_string();
            manifest.extend_from_slice(line.as_bytes());
            manifest.push(b'\n');
            manifest.extend_from_slice(line.as_bytes());
            manifest.push(b'\n');
        }
        let corpus = Manifests(manifest);
        let scratch = Scratch::new();
        let database = Database::open_in(&scratch.0).unwrap();
        let workspace = "workspace/default".parse().unwrap();
        database
            .grant("tester", &workspace, Right::Read, "test")
            .unwrap();
        let scopes: ScopeSet = database.visible("tester").unwrap();
        let target = Target {
            database: &database,
            scopes: &scopes,
            collection: "pages",
            source: "web",
        };

        let references = shared_source_refs(&target, &corpus).unwrap();
        assert_eq!(references.len(), REFERENCES);

        let mut report = Report::new("synthetic");
        let mut observer = |_: &Report| ControlFlow::Continue(());
        import_source(&target, &corpus, &mut report, &mut observer).unwrap();
        let refusals = REFERENCES * 2;
        assert_eq!(report.refused, u64::try_from(refusals).unwrap());
        assert_eq!(report.refusals.len(), refusals);
    }
}
