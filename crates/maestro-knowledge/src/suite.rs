//! An evaluation suite: `maestro-suite/1`, one JSON line per question, which
//! the evaluation runner reads (plan D13). A collection's declaration names a
//! directory in `evals.suite`, and each `<name>.jsonl` in it is the suite
//! `<name>`, so one collection can hold several suites.
//!
//! A question names the sections that answer it, never their IDs: a section
//! ID derives from its document's revision, which changes with any metadata
//! the importer supplies, and a person checking a question reads heading
//! paths, not hashes. An expected section is named by its document's
//! `source_ref` and its heading path, the heading texts as canonicalization
//! derives them from the document's first heading down to the section's own,
//! plus a 1-based `occurrence` only when that path repeats in the document.
//! The runner resolves each name in the canonicalized document of the
//! generation it evaluates ([`ExpectedSection::resolve`]), which refuses a
//! name that matches no section, several without an occurrence, or an
//! occurrence where the path does not repeat or repeats fewer times.
//!
//! A suite is strict as a corpus manifest is: every line is one JSON object,
//! never an array of its values; every key is one the contract names and
//! appears once in its object, in each expected section too; `schema` and
//! `language` are written as strings; and an occurrence is a whole number, 1
//! or more. A suite holds at least one question, a question is answerable
//! exactly when it expects a section, and no two questions share an id.

use crate::shape;
use maestro_canonicalization::{CanonicalDocument, Section};
use serde::Deserialize;
use std::{collections::BTreeMap, error, fmt, num::NonZeroU32, str::FromStr};

/// A suite's questions. [`str::parse`] reads them from a text of one JSON
/// object per line, and refuses what each line's shape alone allows: a text
/// without a question, an `answerable` that disagrees with `expected`, and an
/// id given twice.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Suite {
    /// The questions, in the order of their lines.
    pub questions: Vec<Question>,
}

impl FromStr for Suite {
    type Err = Error;

    /// The questions `text` holds, one per line.
    ///
    /// # Errors
    ///
    /// [`Error::Json`] for the first line that is not strict JSON of the
    /// contract's shape, [`Error::Answerable`] for the first whose
    /// `answerable` disagrees with its `expected`, [`Error::DuplicateId`] for
    /// the first that repeats an earlier question's id, and [`Error::Empty`]
    /// for a text without a question.
    fn from_str(text: &str) -> Result<Self, Error> {
        let mut first_lines = BTreeMap::new();
        let mut questions = Vec::new();
        for (line, text) in (1..).zip(text.lines()) {
            let question: Question =
                shape::parse(text).map_err(|error| Error::Json { line, error })?;
            if question.answerable == question.expected.is_empty() {
                return Err(Error::Answerable {
                    line,
                    answerable: question.answerable,
                });
            }
            if let Some(&first) = first_lines.get(&question.id) {
                return Err(Error::DuplicateId {
                    line,
                    first,
                    id: question.id,
                });
            }
            first_lines.insert(question.id.clone(), line);
            questions.push(question);
        }
        if questions.is_empty() {
            return Err(Error::Empty);
        }
        Ok(Self { questions })
    }
}

/// One question of a suite, from one line.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Question {
    /// The contract the line follows.
    #[serde(deserialize_with = "shape::name")]
    pub schema: Schema,
    /// The question's id, unique in its suite.
    pub id: String,
    /// The language the question is asked in.
    #[serde(deserialize_with = "shape::name")]
    pub language: Language,
    /// The question, as a person would ask it.
    pub question: String,
    /// Whether any section answers it.
    pub answerable: bool,
    /// The sections that answer it, none when it is not answerable.
    #[serde(deserialize_with = "shape::objects")]
    pub expected: Vec<ExpectedSection>,
}

/// A section that answers a question, named as a person reads it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ExpectedSection {
    /// Its document's `source_ref`, as the corpus manifest gives it.
    pub source_ref: String,
    /// Its heading path: the heading texts from the document's first heading
    /// down to the section's own.
    pub heading_path: Vec<String>,
    /// Which of the sections under that heading path it is, counted from 1 in
    /// the document's order; given only when the path repeats.
    pub occurrence: Option<NonZeroU32>,
}

impl ExpectedSection {
    /// The section this name gives in `document`, the canonicalized document
    /// whose `source_ref` it names; the caller finds that document. The
    /// heading path must equal a section's whole heading path, text for text.
    ///
    /// # Errors
    ///
    /// [`Unresolved`] when no section has the heading path, when several
    /// have it and no occurrence says which, and when an occurrence is given
    /// for a path that does not repeat or repeats fewer times.
    pub fn resolve<'document>(
        &self,
        document: &'document CanonicalDocument,
    ) -> Result<&'document Section, Unresolved> {
        let matching: Vec<&Section> = document
            .sections
            .iter()
            .filter(|section| section.heading_path == self.heading_path)
            .collect();
        match (self.occurrence, matching.as_slice()) {
            (_, []) => Err(Unresolved::NoSection),
            (None, &[only]) => Ok(only),
            (None, repeated) => Err(Unresolved::Ambiguous {
                sections: repeated.len(),
            }),
            (Some(_), [_]) => Err(Unresolved::NotRepeated),
            (Some(occurrence), repeated) => usize::try_from(occurrence.get() - 1)
                .ok()
                .and_then(|index| repeated.get(index))
                .copied()
                .ok_or(Unresolved::PastLastRepeat {
                    occurrence,
                    sections: repeated.len(),
                }),
        }
    }
}

/// Why an expected section names no one section of its document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    /// No section has the heading path.
    NoSection,
    /// Several sections have the heading path, and no occurrence says which.
    Ambiguous {
        /// How many sections have it.
        sections: usize,
    },
    /// An occurrence is given, but only one section has the heading path.
    NotRepeated,
    /// The occurrence is past the last section with the heading path.
    PastLastRepeat {
        /// The occurrence given.
        occurrence: NonZeroU32,
        /// How many sections have the heading path.
        sections: usize,
    },
}

impl fmt::Display for Unresolved {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSection => formatter.write_str("no section has this heading path"),
            Self::Ambiguous { sections } => write!(
                formatter,
                "{sections} sections have this heading path, and no occurrence says which"
            ),
            Self::NotRepeated => {
                formatter.write_str("one section has this heading path, so it takes no occurrence")
            }
            Self::PastLastRepeat {
                occurrence,
                sections,
            } => write!(
                formatter,
                "occurrence {occurrence} is past the {sections} sections with this heading path"
            ),
        }
    }
}

impl error::Error for Unresolved {}

/// The contract a line follows; this version reads the first only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Schema {
    /// `maestro-suite/1`.
    #[serde(rename = "maestro-suite/1")]
    V1,
}

/// The language a question is asked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// `fr`: French.
    Fr,
    /// `en`: English.
    En,
}

/// Why a text is not a `maestro-suite/1` suite: it holds no question, or one
/// of its lines, named by its 1-based number, breaks the contract.
#[derive(Debug)]
pub enum Error {
    /// The text holds no question.
    Empty,
    /// The line is not strict JSON of the contract's shape: not one JSON
    /// object, an unknown, repeated or missing key, a value the contract does
    /// not allow or an occurrence below 1.
    Json {
        /// The line.
        line: usize,
        /// The parser's reason.
        error: serde_json::Error,
    },
    /// The line's `answerable` disagrees with its `expected`: true with no
    /// expected section, or false with some.
    Answerable {
        /// The line.
        line: usize,
        /// What the line says.
        answerable: bool,
    },
    /// The line repeats the id of an earlier question.
    DuplicateId {
        /// The line.
        line: usize,
        /// The line of the question that holds the id first.
        first: usize,
        /// The repeated id.
        id: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str(
                "a maestro-suite/1 suite holds at least one question, \
                 and this text holds no question",
            ),
            Self::Json { line, error } => write!(
                formatter,
                "line {line} is not a strict maestro-suite/1 question: {error}"
            ),
            Self::Answerable { line, answerable } => write!(
                formatter,
                "line {line} says answerable is {answerable}, but a question is answerable \
                 exactly when it expects a section"
            ),
            Self::DuplicateId { line, first, id } => write!(
                formatter,
                "line {line} repeats the id `{id}` of line {first}"
            ),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Json { error, .. } => Some(error),
            Self::Empty | Self::Answerable { .. } | Self::DuplicateId { .. } => None,
        }
    }
}
