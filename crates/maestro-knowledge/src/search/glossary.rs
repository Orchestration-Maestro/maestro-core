//! A query expander over a collection's glossary: a strict, size-bounded
//! `maestro-glossary/1` JSON table that maps the words people ask with, such
//! as "third failure", to the words the documentation uses, such as
//! `NumberOfFailures`. Matching is literal: whole words, without case or
//! accents, and never on its own output. The product ships no glossary; a
//! machine binds one per collection, and a run pins it by its digest.

use super::intent::{Expansion, ExpansionFailure, ExpansionFuture, QueryExpander};
use crate::{lexical, query::Understood, shape};
use maestro_kernel::artifact::Digest;
use serde::Deserialize;
use std::{collections::BTreeSet, error, fmt, future::ready, str};

/// The contract a glossary follows.
const SCHEMA: &str = "maestro-glossary/1";
/// The largest glossary file, in bytes.
pub const MAX_GLOSSARY_BYTES: usize = 256 * 1024;
/// The most entries of a glossary.
const MAX_ENTRIES: usize = 1024;
/// The most phrases, context phrases or added terms of one entry.
const MAX_LIST: usize = 16;
/// The longest ID, phrase, context phrase or term, in bytes.
const MAX_TEXT_BYTES: usize = 128;
/// The most terms one expansion adds.
const MAX_ADDED_TERMS: usize = 8;
/// The longest added text, the expansion guard's keyword bound.
const MAX_ADDED_BYTES: usize = 512;

/// A glossary as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    /// `maestro-glossary/1`.
    schema: String,
    /// The entries, matched in order.
    #[serde(deserialize_with = "shape::objects")]
    entries: Vec<WrittenEntry>,
}

/// An entry as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrittenEntry {
    /// Its stable ID, which traces name.
    id: String,
    /// The phrases people ask with, any of which matches.
    phrases: Vec<String>,
    /// Phrases one of which the question must also hold; empty for none.
    #[serde(default)]
    requires_any: Vec<String>,
    /// The documentation's words a match adds.
    add: Vec<String>,
}

/// A checked entry.
#[derive(Debug)]
struct Entry {
    /// Each phrase's words, folded and lower-cased.
    phrases: Vec<Vec<String>>,
    /// Each context phrase's words, folded and lower-cased.
    requires_any: Vec<Vec<String>>,
    /// The terms it adds, as written.
    add: Vec<String>,
}

/// A checked glossary and the digest of its bytes.
#[derive(Debug)]
pub struct Glossary {
    /// Its entries, in order.
    entries: Vec<Entry>,
    /// The SHA-256 of the bytes it was read from.
    digest: Digest,
}

impl Glossary {
    /// The glossary `bytes` hold.
    ///
    /// # Errors
    ///
    /// [`GlossaryError`] when the bytes are over [`MAX_GLOSSARY_BYTES`], are
    /// not strict JSON of the contract, break one of its bounds, or give one
    /// phrase to two entries.
    pub fn parse(bytes: &[u8]) -> Result<Self, GlossaryError> {
        if bytes.len() > MAX_GLOSSARY_BYTES {
            return Err(GlossaryError::TooLarge);
        }
        let text = str::from_utf8(bytes).map_err(|_| GlossaryError::Encoding)?;
        let written: Written = shape::parse(text).map_err(GlossaryError::Json)?;
        if written.schema != SCHEMA {
            return Err(GlossaryError::Schema(written.schema));
        }
        if written.entries.len() > MAX_ENTRIES {
            return Err(GlossaryError::Bounds);
        }
        let mut owners = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut entries = Vec::with_capacity(written.entries.len());
        for entry in written.entries {
            if !ids.insert(entry.id.clone()) {
                return Err(GlossaryError::DuplicateId(entry.id));
            }
            let checked = Entry::checked(entry)?;
            if let Some(phrase) = checked
                .phrases
                .iter()
                .find(|phrase| !owners.insert((*phrase).clone()))
            {
                return Err(GlossaryError::AmbiguousPhrase(phrase.join(" ")));
            }
            entries.push(checked);
        }
        Ok(Self {
            entries,
            digest: Digest::of(bytes),
        })
    }

    /// The SHA-256 of the bytes it was read from, which runs pin.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    /// The terms the entries matching `text` add, in entry order, each once,
    /// none `text` already holds, within the expansion's bounds.
    #[must_use]
    pub fn bridge(&self, text: &str) -> Vec<&str> {
        let question = words(text);
        let mut added: Vec<&str> = Vec::new();
        let mut bytes = 0;
        let matching = self.entries.iter().filter(|entry| entry.matches(&question));
        for term in matching.flat_map(|entry| entry.add.iter()) {
            if added.len() == MAX_ADDED_TERMS {
                break;
            }
            let length = bytes + usize::from(!added.is_empty()) + term.len();
            if added.contains(&term.as_str())
                || holds(&question, &words(term))
                || length > MAX_ADDED_BYTES
            {
                continue;
            }
            bytes = length;
            added.push(term);
        }
        added
    }
}

impl QueryExpander for Glossary {
    fn expand<'a>(&'a self, question: &'a Understood) -> ExpansionFuture<'a> {
        let terms = self.bridge(&question.normalized);
        Box::pin(ready(if terms.is_empty() {
            Err(ExpansionFailure::NoMatch)
        } else {
            let keywords = terms.join(" ");
            Ok(Expansion {
                passage: format!("{} {keywords}", question.normalized),
                keywords,
            })
        }))
    }
}

impl Entry {
    /// The entry `entry` writes, once each list is within its bound, each
    /// text is visible, one line and short, and each phrase and context
    /// phrase has a word.
    fn checked(entry: WrittenEntry) -> Result<Self, GlossaryError> {
        let lists = [&entry.phrases, &entry.requires_any, &entry.add];
        if entry.phrases.is_empty()
            || entry.add.is_empty()
            || lists.iter().any(|list| list.len() > MAX_LIST)
        {
            return Err(GlossaryError::Bounds);
        }
        for text in lists.into_iter().flatten().chain([&entry.id]) {
            check_text(text)?;
        }
        let phrases = entry
            .phrases
            .iter()
            .map(|phrase| words(phrase))
            .collect::<Vec<_>>();
        let requires_any = entry
            .requires_any
            .iter()
            .map(|phrase| words(phrase))
            .collect::<Vec<_>>();
        if phrases.iter().chain(&requires_any).any(Vec::is_empty) {
            return Err(GlossaryError::NoWords);
        }
        Ok(Self {
            phrases,
            requires_any,
            add: entry.add,
        })
    }

    /// Whether `words` hold one of its phrases, and one of its context
    /// phrases when it has some.
    fn matches(&self, words: &[String]) -> bool {
        self.phrases.iter().any(|phrase| holds(words, phrase))
            && (self.requires_any.is_empty()
                || self.requires_any.iter().any(|phrase| holds(words, phrase)))
    }
}

/// Refuses a blank text, one with a control character, or one over
/// [`MAX_TEXT_BYTES`].
fn check_text(text: &str) -> Result<(), GlossaryError> {
    if text.trim().is_empty() || text.chars().any(char::is_control) || text.len() > MAX_TEXT_BYTES {
        return Err(GlossaryError::BlankText);
    }
    Ok(())
}

/// The words of `text`, folded and lower-cased: its runs of letters and
/// digits.
fn words(text: &str) -> Vec<String> {
    lexical::fold(text)
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether `words` hold `phrase` as consecutive words.
fn holds(words: &[String], phrase: &[String]) -> bool {
    !phrase.is_empty() && words.windows(phrase.len()).any(|window| window == phrase)
}

/// Why bytes are not a glossary.
#[derive(Debug)]
pub enum GlossaryError {
    /// Over [`MAX_GLOSSARY_BYTES`].
    TooLarge,
    /// Not UTF-8.
    Encoding,
    /// Not strict JSON of the contract's shape.
    Json(serde_json::Error),
    /// A schema other than `maestro-glossary/1`.
    Schema(String),
    /// Too many entries, or a list empty or too long.
    Bounds,
    /// An ID given to two entries.
    DuplicateId(String),
    /// A text that is blank, holds a control character or is too long.
    BlankText,
    /// A phrase or a context phrase without a word.
    NoWords,
    /// A phrase two entries give.
    AmbiguousPhrase(String),
}

impl fmt::Display for GlossaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(formatter, "the glossary is over {MAX_GLOSSARY_BYTES} bytes"),
            Self::Encoding => formatter.write_str("the glossary is not UTF-8"),
            Self::Json(error) => write!(formatter, "the glossary is invalid: {error}"),
            Self::Schema(schema) => write!(formatter, "unknown glossary schema {schema:?}"),
            Self::Bounds => formatter.write_str("a glossary list is empty or over its bound"),
            Self::DuplicateId(id) => write!(formatter, "the glossary gives the ID {id:?} twice"),
            Self::BlankText => formatter
                .write_str("a glossary text is blank, too long or holds a control character"),
            Self::NoWords => formatter.write_str("a glossary phrase has no word"),
            Self::AmbiguousPhrase(phrase) => {
                write!(formatter, "two glossary entries give the phrase {phrase:?}")
            }
        }
    }
}

impl error::Error for GlossaryError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::TooLarge
            | Self::Encoding
            | Self::Schema(_)
            | Self::Bounds
            | Self::DuplicateId(_)
            | Self::BlankText
            | Self::NoWords
            | Self::AmbiguousPhrase(_) => None,
        }
    }
}
