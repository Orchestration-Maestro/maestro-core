//! Scans explicit outgoing Git objects against authenticated fingerprints.

use super::{
    bank::Bank,
    git_objects::{self, Object},
    lookup::LookupStatements,
    normalize::{
        SHINGLE_LENGTH, SHINGLE_SIZE, SHORT_UNIT_TAG, Token, json_strings, shingle_tag,
        short_unit_tag, tag, tokens,
    },
};
use std::{error::Error, path::Path, result::Result as StdResult};

/// Kind tag for exact source-file fingerprints.
const FILE_TAG: u8 = 1;
/// Kind tag for normalized token-shingle fingerprints.
const SHINGLE_TAG: u8 = 2;
/// Maximum number of detailed locations printed for one scan.
const DETAIL_LIMIT: usize = 100;

/// Internal error type for Git scanning operations.
type Result<T> = StdResult<T, Box<dyn Error>>;

/// One outgoing source ref, its optional comparison base, and selected remote.
#[derive(Debug)]
pub(super) struct RefSpec {
    /// Full object ID at the outgoing ref tip.
    source: String,
    /// Full object ID at the old ref tip, or no base for a new ref.
    base: Option<String>,
    /// Local remote whose already-pushed refs are excluded from the scan.
    remote: String,
}

/// Validated external bank paths and expected source snapshot.
#[derive(Clone, Copy)]
pub(super) struct BankFiles<'a> {
    /// Read-only SQLite database.
    pub(super) bank: &'a Path,
    /// Keyed HMAC-SHA-256 of the complete database file.
    pub(super) digest: &'a Path,
    /// Mode-600 HMAC key.
    pub(super) key: &'a Path,
    /// Opaque inventory identifier bound into the database.
    pub(super) inventory: &'a str,
}

/// Aggregate counts and bounded location-only findings.
#[derive(Debug)]
pub(super) struct ScanReport {
    /// Number of outgoing refspecs scanned.
    pub refs: usize,
    /// Number of Git objects read.
    pub objects: u64,
    /// Number of matching locations.
    pub hits: u64,
    /// At most [`DETAIL_LIMIT`] locations, without matched content.
    pub details: Vec<Finding>,
}

/// A matching object location, never containing matched text.
#[derive(Debug)]
pub(super) struct Finding {
    /// One-based index of the outgoing refspec.
    pub reference: usize,
    /// Full Git object ID containing the match.
    pub object: String,
    /// Generic object unit such as `blob` or `message`.
    pub unit: String,
    /// One-based line number, or zero when not applicable.
    pub line: usize,
    /// Zero-based byte offset of the match start.
    pub start: usize,
    /// Exclusive byte offset of the match end.
    pub end: usize,
}

/// Half-open byte range of one candidate match.
#[derive(Clone, Copy)]
struct Span {
    /// Inclusive start offset.
    start: usize,
    /// Exclusive end offset.
    end: usize,
}

/// State shared while one explicit outgoing ref is scanned.
struct Scanner<'scan, 'connection> {
    /// Repository used for Git plumbing calls.
    repository: &'scan Path,
    /// One-based refspec index used in findings.
    reference: usize,
    /// Reusable prepared lookups into the authenticated bank.
    lookups: &'scan mut LookupStatements<'connection>,
    /// Aggregate location-only output.
    report: &'scan mut ScanReport,
}

impl RefSpec {
    /// Parses a full-OID `source:base` pair; `-` marks a newly created ref.
    pub(super) fn parse(value: &str, remote: &str) -> Result<Self> {
        let (source, base) = value.split_once(':').ok_or_else(git_objects::invalid)?;
        if base.contains(':')
            || !git_objects::valid_oid(source)
            || (base != "-" && !git_objects::valid_oid(base))
        {
            return Err(git_objects::invalid().into());
        }
        Ok(Self {
            source: source.to_owned(),
            base: (base != "-").then(|| base.to_owned()),
            remote: remote.to_owned(),
        })
    }
}

/// Scans explicit outgoing refspec object ranges against a validated bank.
pub(super) fn scan(
    repository: &Path,
    references: &[RefSpec],
    bank_files: BankFiles<'_>,
) -> Result<ScanReport> {
    let first = references.first().ok_or_else(git_objects::invalid)?;
    git_objects::check_remote(repository, &first.remote)?;
    let bank = Bank::open(
        bank_files.bank,
        bank_files.key,
        bank_files.digest,
        bank_files.inventory,
    )?;
    let mut lookups = bank.lookup_statements()?;
    if git_objects::shallow(repository)? {
        return Err(git_objects::invalid().into());
    }
    let mut report = ScanReport {
        refs: references.len(),
        objects: 0,
        hits: 0,
        details: Vec::new(),
    };
    for (index, reference) in references.iter().enumerate() {
        scan_reference(repository, reference, index + 1, &mut lookups, &mut report)?;
    }
    Ok(report)
}

/// Reads and scans one source ref against its old tip.
fn scan_reference(
    repository: &Path,
    reference: &RefSpec,
    index: usize,
    lookups: &mut LookupStatements<'_>,
    report: &mut ScanReport,
) -> Result<()> {
    let source_type = git_objects::object_type(repository, &reference.source)?;
    if source_type != "commit" && source_type != "tag" {
        return Err(git_objects::invalid().into());
    }
    if let Some(base) = &reference.base {
        let base_type = git_objects::object_type(repository, base)?;
        if base_type != "commit" && base_type != "tag" {
            return Err(git_objects::invalid().into());
        }
    }
    if source_type == "tag" {
        git_objects::peeled_commit(repository, &reference.source)?;
    }
    let mut object_ids = git_objects::revision_objects(
        repository,
        &reference.source,
        reference.base.as_deref(),
        &reference.remote,
    )?;
    if source_type == "tag" {
        object_ids.insert(reference.source.clone());
    }
    let objects = git_objects::read_objects(repository, &object_ids)?;
    let commits: Vec<String> = objects
        .iter()
        .filter(|object| object.kind == "commit")
        .map(|object| object.oid.clone())
        .collect();
    let mut scanner = Scanner {
        repository,
        reference: index,
        lookups,
        report,
    };
    for object in &objects {
        scanner.report.objects = scanner.report.objects.saturating_add(1);
        match object.kind.as_str() {
            "blob" => scanner.scan_blob(&object.oid, &object.bytes)?,
            "commit" | "tag" => scanner.scan_message(object)?,
            _ => return Err(git_objects::invalid().into()),
        }
    }
    for commit_id in &commits {
        scanner.scan_tree(commit_id)?;
    }
    Ok(())
}

impl Scanner<'_, '_> {
    /// Scans paths in a commit introduced by the outgoing ref.
    fn scan_tree(&mut self, commit: &str) -> Result<()> {
        for (ordinal, entry) in git_objects::tree_entries(self.repository, commit)?
            .iter()
            .enumerate()
        {
            let name = String::from_utf8_lossy(&entry.path);
            self.scan_text(commit, &format!("path-{ordinal}"), &name, false)?;
        }
        Ok(())
    }

    /// Checks an exact blob fingerprint, then scans text and decoded JSON strings.
    fn scan_blob(&mut self, oid: &str, bytes: &[u8]) -> Result<()> {
        if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n") {
            return Err(git_objects::invalid().into());
        }
        let length = u64::try_from(bytes.len())?;
        let exact = tag(self.lookups.key(), FILE_TAG, length, bytes);
        if self.lookups.contains(FILE_TAG, length, &exact)? {
            self.record(
                oid,
                "blob",
                Span {
                    start: 0,
                    end: bytes.len(),
                },
                0,
            );
            return Ok(());
        }
        self.scan_text(oid, "blob", &String::from_utf8_lossy(bytes), true)?;
        for (unit, value) in json_strings(bytes)?.iter().enumerate() {
            self.scan_text(oid, &format!("json-{unit}"), value, false)?;
        }
        Ok(())
    }

    /// Scans the message portion of a commit or annotated tag object.
    fn scan_message(&mut self, object: &Object) -> Result<()> {
        let message = object
            .bytes
            .windows(2)
            .position(|window| window == b"\n\n")
            .and_then(|offset| offset.checked_add(2))
            .and_then(|start| object.bytes.get(start..))
            .unwrap_or_default();
        self.scan_text(
            &object.oid,
            "message",
            &String::from_utf8_lossy(message),
            true,
        )
    }

    /// Finds and merges matching shingle ranges without retaining their text.
    fn scan_text(&mut self, oid: &str, unit: &str, text: &str, line_numbers: bool) -> Result<()> {
        let words = tokens(text);
        let mut spans = Vec::new();
        for window in words.windows(SHINGLE_SIZE) {
            let fingerprint = shingle_tag(self.lookups.key(), window);
            if self
                .lookups
                .contains(SHINGLE_TAG, SHINGLE_LENGTH, &fingerprint)?
            {
                let first = window.first().ok_or_else(git_objects::invalid)?;
                let last = window.last().ok_or_else(git_objects::invalid)?;
                spans.push(Span {
                    start: first.start,
                    end: last.end,
                });
            }
        }
        spans.extend(self.short_unit_spans(&words)?);
        spans.sort_by_key(|span| span.start);
        let mut merged: Vec<Span> = Vec::new();
        for span in spans {
            if let Some(previous) = merged.last_mut()
                && span.start <= previous.end
            {
                previous.end = previous.end.max(span.end);
            } else {
                merged.push(span);
            }
        }
        for span in merged {
            let line = if line_numbers {
                text.get(..span.start)
                    .ok_or_else(git_objects::invalid)?
                    .lines()
                    .count()
                    .saturating_add(1)
            } else {
                0
            };
            self.record(oid, unit, span, line);
        }
        Ok(())
    }

    /// Finds all non-allowlisted exact private units of four to seven tokens.
    fn short_unit_spans(&mut self, words: &[Token]) -> Result<Vec<Span>> {
        let mut spans = Vec::new();
        for size in 4..SHINGLE_SIZE {
            spans.append(&mut self.short_unit_spans_of_size(words, size)?);
        }
        Ok(spans)
    }

    /// Finds matching private units of one token length.
    fn short_unit_spans_of_size(&mut self, words: &[Token], size: usize) -> Result<Vec<Span>> {
        let length = u64::try_from(size)?;
        let mut spans = Vec::new();
        for window in words.windows(size) {
            if self.short_unit_matches(window, length)? {
                let first = window.first().ok_or_else(git_objects::invalid)?;
                let last = window.last().ok_or_else(git_objects::invalid)?;
                spans.push(Span {
                    start: first.start,
                    end: last.end,
                });
            }
        }
        Ok(spans)
    }

    /// Checks a candidate against the bank and exact-unit allowlist.
    fn short_unit_matches(&mut self, words: &[Token], length: u64) -> Result<bool> {
        let fingerprint = short_unit_tag(self.lookups.key(), words);
        Ok(self
            .lookups
            .contains(SHORT_UNIT_TAG, length, &fingerprint)?
            && !self.lookups.unit_allowed(length, &fingerprint)?)
    }

    /// Adds one location and increments the total hit count.
    fn record(&mut self, object: &str, unit: &str, span: Span, line: usize) {
        self.report.hits = self.report.hits.saturating_add(1);
        if self.report.details.len() < DETAIL_LIMIT {
            self.report.details.push(Finding {
                reference: self.reference,
                object: object.to_owned(),
                unit: unit.to_owned(),
                line,
                start: span.start,
                end: span.end,
            });
        }
    }
}
