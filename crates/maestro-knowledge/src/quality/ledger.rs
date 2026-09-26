//! A collection's quality ledger: `maestro-quality-ledger/1`, the private,
//! versioned rules people write about the collection's revisions
//! (docs/architecture/01 §4's collection rules), one strict JSON rule a line,
//! such as this one, shown here over several lines:
//!
//! ```json
//! {"schema": "maestro-quality-ledger/1", "id": "superseded-release",
//!  "match": {"version": "9.0.21"}, "disposition": "excluded",
//!  "reason": "9.0.22 supersedes it", "decided_by": "the owner",
//!  "reversal": "remove this rule before the gate decides what it matches"}
//! ```
//!
//! - `id`: the rule's ID, unique in its ledger, which follows the rule of
//!   scope names ([`check_name`](maestro_kernel::scope::check_name)); a
//!   disposition the rule gives names it `ledger.<id>`.
//! - `match`: the fields of a corpus entry (`maestro-corpus/1`) the rule
//!   matches, at least one, each equal to the text given: `source_ref`,
//!   `sha256`, `title`, `source_kind`, `set`, `version`, `lang`,
//!   `captured_at`, `product`, `component` and `platform`. A revision
//!   matches when each field named holds that text. `path` is not one: the
//!   kernel does not record where a file sat in its corpus, so a rule names
//!   a document by its `source_ref`.
//! - `disposition`: the outcome the rule gives, one of the five of 01 §4.
//! - `reason`, `decided_by` and `reversal`: why, who decided it and how to
//!   reverse it, each a text that is not blank.
//!
//! A ledger is strict as a corpus manifest is: each line is one JSON object
//! holding these keys only, each once, a `match` object included; each
//! value has its key's shape, a named one written as a string; and no two
//! rules share an ID. A blank line is refused too; the text after the last
//! newline is a line when it is not empty. A missing ledger reads as an
//! empty one ([`Ledger::load`]).
//!
//! The first rule that matches a revision decides it. A rule decides the
//! revisions the gate has not decided yet: a disposition, once recorded, is
//! kept, so a rule written or removed later changes no revision decided
//! before it.

use super::outcome;
use crate::shape;
use maestro_kernel::{
    artifact::Digest,
    document::{Outcome, Revision},
};
use serde::{Deserialize, Deserializer, de};
use serde_json::Value;
use std::{
    error, fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

/// A collection's quality ledger: its rules, in the order of their lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    /// The rules, in the order of their lines.
    rules: Vec<Rule>,
}

impl Ledger {
    /// The ledger in the file `path`, or an empty one when the file does not
    /// exist.
    ///
    /// # Errors
    ///
    /// [`LedgerError::Io`] when the file exists but cannot be read, and the
    /// refusals of [`Ledger::from_str`] for its text.
    pub fn load(path: &Path) -> Result<Self, LedgerError> {
        match fs::read_to_string(path) {
            Ok(text) => text.parse(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(LedgerError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// The rules, in the order of their lines.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The first rule that matches `candidate`, if one does.
    pub(super) fn first_match(&self, candidate: &Candidate<'_>) -> Option<&Rule> {
        self.rules.iter().find(|rule| {
            rule.matches
                .fields()
                .all(|(name, text)| candidate.field(name) == Some(text))
        })
    }
}

impl FromStr for Ledger {
    type Err = LedgerError;

    /// The ledger `text` holds, one rule a line.
    ///
    /// # Errors
    ///
    /// For the first line that is no rule: [`LedgerError::Json`] when it is
    /// not strict JSON of the contract's shape, [`LedgerError::EmptyMatch`]
    /// when its rule matches no field, and [`LedgerError::DuplicateId`] when
    /// an earlier rule has its ID.
    fn from_str(text: &str) -> Result<Self, LedgerError> {
        let mut rules: Vec<Rule> = Vec::new();
        for (line, text) in (1..).zip(text.split_terminator('\n')) {
            let rule: Rule =
                shape::parse(text).map_err(|error| LedgerError::Json { line, error })?;
            if rule.matches.fields().next().is_none() {
                return Err(LedgerError::EmptyMatch { line });
            }
            if rules.iter().any(|earlier| earlier.id == rule.id) {
                return Err(LedgerError::DuplicateId { line, id: rule.id });
            }
            rules.push(rule);
        }
        Ok(Self { rules })
    }
}

/// A rule of a quality ledger: the revisions it matches, the disposition it
/// gives them, why, who decided it and how to reverse it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Rule {
    /// The contract the line follows.
    #[serde(deserialize_with = "shape::name")]
    pub schema: Schema,
    /// The rule's ID, unique in its ledger.
    #[serde(deserialize_with = "shape::id")]
    pub id: String,
    /// The fields of a corpus entry it matches.
    #[serde(rename = "match", deserialize_with = "shape::object")]
    pub matches: Match,
    /// The outcome it gives the revisions it matches.
    #[serde(deserialize_with = "named_outcome")]
    pub disposition: Outcome,
    /// Why, for people.
    #[serde(deserialize_with = "text")]
    pub reason: String,
    /// Who decided it.
    #[serde(deserialize_with = "text")]
    pub decided_by: String,
    /// How to reverse it.
    #[serde(deserialize_with = "text")]
    pub reversal: String,
}

impl Rule {
    /// The rule ID a disposition this rule gives names: `ledger.<id>`.
    #[must_use]
    pub fn rule_id(&self) -> String {
        format!("ledger.{}", self.id)
    }
}

/// The contract a ledger line follows; this version reads the first only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Schema {
    /// `maestro-quality-ledger/1`.
    #[serde(rename = "maestro-quality-ledger/1")]
    V1,
}

/// The fields of a corpus entry a rule matches, each with the text it must
/// hold; at least one is named.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Match {
    /// The document's identity: its origin URL or its corpus path.
    #[serde(default, deserialize_with = "some_text")]
    pub source_ref: Option<String>,
    /// The SHA-256 digest of the revision's original bytes.
    #[serde(default, deserialize_with = "some_digest")]
    pub sha256: Option<Digest>,
    /// The document's title.
    #[serde(default, deserialize_with = "some_text")]
    pub title: Option<String>,
    /// The kind of source it comes from.
    #[serde(default, deserialize_with = "some_text")]
    pub source_kind: Option<String>,
    /// The document set it belongs to.
    #[serde(default, deserialize_with = "some_text")]
    pub set: Option<String>,
    /// The release it documents.
    #[serde(default, deserialize_with = "some_text")]
    pub version: Option<String>,
    /// Its language.
    #[serde(default, deserialize_with = "some_text")]
    pub lang: Option<String>,
    /// When it was captured.
    #[serde(default, deserialize_with = "some_text")]
    pub captured_at: Option<String>,
    /// The product it documents.
    #[serde(default, deserialize_with = "some_text")]
    pub product: Option<String>,
    /// The component it documents.
    #[serde(default, deserialize_with = "some_text")]
    pub component: Option<String>,
    /// The platform it applies to.
    #[serde(default, deserialize_with = "some_text")]
    pub platform: Option<String>,
}

impl Match {
    /// Each field it names, by its corpus entry's name, with its text.
    fn fields(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            ("source_ref", self.source_ref.as_deref()),
            ("sha256", self.sha256.as_ref().map(Digest::as_str)),
            ("title", self.title.as_deref()),
            ("source_kind", self.source_kind.as_deref()),
            ("set", self.set.as_deref()),
            ("version", self.version.as_deref()),
            ("lang", self.lang.as_deref()),
            ("captured_at", self.captured_at.as_deref()),
            ("product", self.product.as_deref()),
            ("component", self.component.as_deref()),
            ("platform", self.platform.as_deref()),
        ]
        .into_iter()
        .filter_map(|(name, text)| Some((name, text?)))
    }
}

/// What a rule matches: a revision, with the `source_ref` of its document.
#[derive(Debug, Clone, Copy)]
pub(super) struct Candidate<'a> {
    /// The revision.
    revision: &'a Revision,
    /// The `source_ref` of its document.
    source_ref: &'a str,
}

impl<'a> Candidate<'a> {
    /// The candidate `revision`, whose document has `source_ref`.
    pub(super) fn new(revision: &'a Revision, source_ref: &'a str) -> Self {
        Self {
            revision,
            source_ref,
        }
    }

    /// The revision.
    pub(super) fn revision(&self) -> &'a Revision {
        self.revision
    }

    /// The text of the corpus entry's field `name`: `source_ref` from the
    /// document, `sha256` the digest of the original bytes, and every other
    /// from the revision's metadata, which the import names as the manifest
    /// does.
    fn field(&self, name: &str) -> Option<&str> {
        match name {
            "source_ref" => Some(self.source_ref),
            "sha256" => Some(self.revision.original_digest.as_str()),
            _ => self.revision.metadata.get(name).and_then(Value::as_str),
        }
    }
}

/// A text that is not blank.
fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let text = String::deserialize(deserializer)?;
    if text.trim().is_empty() {
        Err(de::Error::custom("a blank text, where a rule needs one"))
    } else {
        Ok(text)
    }
}

/// A text that is not blank, which a field of `match` must hold.
fn some_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    text(deserializer).map(Some)
}

/// The digest `sha256` must hold: 64 lowercase hexadecimal characters.
fn some_digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Digest>, D::Error> {
    let text = String::deserialize(deserializer)?;
    Digest::parse(&text).map(Some).map_err(de::Error::custom)
}

/// An outcome, by its name.
fn named_outcome<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Outcome, D::Error> {
    let text = String::deserialize(deserializer)?;
    outcome::ALL
        .into_iter()
        .find(|named| outcome::name(*named) == text)
        .ok_or_else(|| {
            let names: Vec<&str> = outcome::ALL.into_iter().map(outcome::name).collect();
            de::Error::custom(format_args!(
                "unknown disposition `{text}`, expected one of {}",
                names.join(", ")
            ))
        })
}

/// Why a quality ledger was refused.
#[derive(Debug)]
pub enum LedgerError {
    /// The ledger exists but cannot be read.
    Io {
        /// The ledger's file.
        path: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },
    /// This line is not a strict `maestro-quality-ledger/1` rule: not one
    /// JSON object, a key unknown, repeated or missing, a value of another
    /// shape than its key's, a blank text, a field `match` cannot name, or
    /// an ID that is not a scope name.
    Json {
        /// The line's number, from 1.
        line: u64,
        /// What is wrong with it.
        error: serde_json::Error,
    },
    /// The rule of this line matches no field.
    EmptyMatch {
        /// The line's number, from 1.
        line: u64,
    },
    /// The rule of this line has the ID of an earlier one.
    DuplicateId {
        /// The line's number, from 1.
        line: u64,
        /// The ID.
        id: String,
    },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, .. } => {
                write!(
                    formatter,
                    "cannot read the quality ledger {}",
                    path.display()
                )
            }
            Self::Json { line, error } => write!(
                formatter,
                "line {line} of the quality ledger is not a strict maestro-quality-ledger/1 \
                 rule: {error}"
            ),
            Self::EmptyMatch { line } => write!(
                formatter,
                "line {line} of the quality ledger matches no field: a rule names at least one \
                 field of a corpus entry"
            ),
            Self::DuplicateId { line, id } => write!(
                formatter,
                "line {line} of the quality ledger gives the rule ID `{id}` of an earlier line"
            ),
        }
    }
}

impl error::Error for LedgerError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { error, .. } => Some(error),
            Self::EmptyMatch { .. } | Self::DuplicateId { .. } => None,
        }
    }
}
