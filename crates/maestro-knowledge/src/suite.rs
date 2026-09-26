//! An evaluation suite: `maestro-suite/1`, one JSON line per question, which
//! the evaluation runner reads (plan D13).
//!
//! A question names the sections that answer it, never their IDs: a section
//! ID derives from its document's revision, which changes with any metadata
//! the importer supplies, and a person checking a question reads heading
//! paths, not hashes. An expected section is named by its document's
//! `source_ref` and its heading path, the heading texts as canonicalization
//! derives them from the document's first heading down to the section's own,
//! plus a 1-based `occurrence` only when that path repeats in the document.
//! The runner resolves each name to the section IDs of the generation it
//! evaluates, and it is there that a name which matches no section, or
//! several without an occurrence, is refused.
//!
//! A suite is strict as a corpus manifest is: every line is one JSON object,
//! never an array of its values; every key is one the contract names and
//! appears once in its object, in each expected section too; `schema` and
//! `language` are written as strings; and an occurrence is a whole number, 1
//! or more. A question is answerable exactly when it expects a section, and no
//! two questions share an id.

use crate::shape;
use serde::Deserialize;
use std::{collections::BTreeMap, error, fmt, num::NonZeroU32, str::FromStr};

/// A suite's questions. [`str::parse`] reads them from a text of one JSON
/// object per line, and refuses what each line's shape alone allows: an
/// `answerable` that disagrees with `expected`, and an id given twice.
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
    /// `answerable` disagrees with its `expected`, and [`Error::DuplicateId`]
    /// for the first that repeats an earlier question's id.
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

/// Why a text is not a `maestro-suite/1` suite, each reason with its 1-based
/// line.
#[derive(Debug)]
pub enum Error {
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
            Self::Answerable { .. } | Self::DuplicateId { .. } => None,
        }
    }
}
