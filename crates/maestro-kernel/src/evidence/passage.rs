//! The passages a bundle cites: the source text of a span of one revision,
//! with what identifies and cites it.

use crate::artifact::Digest;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::fmt;

/// A passage a bundle cites: the exact source text of a span of one
/// revision, as the authority holds it, with what identifies and cites it.
/// How it was found and ranked is in the bundle's trace, never here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Passage {
    /// Its number in the bundle, from 1: conflicts, the trace and an
    /// answer's citations name it by this number.
    pub n: u32,
    /// The section its text belongs to, if it belongs to one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    /// The document of its revision, written `doc_id`.
    #[serde(rename = "doc_id")]
    pub document_id: String,
    /// The revision whose original Markdown holds its text.
    pub revision_id: String,
    /// The title of its document.
    pub title: String,
    /// The headings above its text, the outermost first.
    pub section_path: Vec<String>,
    /// The version its revision documents, if it states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Where its document comes from: its origin URL, or `corpus-path:` and
    /// its path when it has none.
    pub source_ref: String,
    /// Where its text lies in the revision's original Markdown.
    pub span: Span,
    /// The SHA-256 of its text, written `sha256:` and 64 lowercase
    /// hexadecimal characters.
    #[serde(serialize_with = "write_digest", deserialize_with = "read_digest")]
    pub digest: Digest,
    /// Its text, verbatim.
    pub text: String,
    /// The same section in other versions: listed, not ranked.
    pub alternates: Vec<Alternate>,
}

/// The same section in another version, listed beside a passage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alternate {
    /// The version it documents, if it states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Its section.
    pub section_id: String,
}

/// A span of a revision's original Markdown in UTF-8 bytes, its start
/// included and its end excluded. It is written `[start, end]`, and read, as
/// a bundle writes it, only when its start is not after its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "[usize; 2]", try_from = "[usize; 2]")]
pub struct Span {
    /// The offset of its first byte.
    pub start: usize,
    /// The offset just past its last byte.
    pub end: usize,
}

impl Span {
    /// The span, unless it starts after it ends, which no span of text does.
    pub(super) fn checked(self) -> Result<Self, String> {
        if self.start <= self.end {
            Ok(self)
        } else {
            Err(format!("the span {self} starts after it ends"))
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "[{}, {})", self.start, self.end)
    }
}

impl From<Span> for [usize; 2] {
    fn from(span: Span) -> Self {
        [span.start, span.end]
    }
}

impl TryFrom<[usize; 2]> for Span {
    type Error = String;

    /// The span from `start` to `end`, unless it starts after it ends.
    fn try_from([start, end]: [usize; 2]) -> Result<Self, String> {
        Self { start, end }.checked()
    }
}

/// Writes `digest` as `sha256:` and its 64 hexadecimal characters.
fn write_digest<S: Serializer>(digest: &Digest, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&format_args!("sha256:{}", digest.as_str()))
}

/// Reads a digest written `sha256:` and 64 lowercase hexadecimal characters,
/// and nothing else.
fn read_digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Digest, D::Error> {
    let text = String::deserialize(deserializer)?;
    text.strip_prefix("sha256:")
        .and_then(|hex| Digest::parse(hex).ok())
        .ok_or_else(|| {
            D::Error::custom(format!(
                "not a digest written sha256: and 64 lowercase hexadecimal characters: {text:?}"
            ))
        })
}
