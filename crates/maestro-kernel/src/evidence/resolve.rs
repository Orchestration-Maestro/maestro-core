//! Resolving a chunk: the exact source text its span covers, read from the
//! authority, with its digest, its span, its version and what identifies it.

use super::{error::Error, passage::Span};
use crate::{
    artifact::{self, Digest},
    scope::ScopeSet,
    store::{self, Database},
};
use rusqlite::{OptionalExtension as _, Row, params, types::Type};
use std::str;

/// Finds a chunk of a chunk set with its revision and document: the columns
/// [`located`] reads, in its order. The version is the revision's `version`
/// metadata when it is text. The caller's scopes follow, as a condition on
/// the scope of the document's source.
const LOCATE: &str = "SELECT chunks.revision_id, revisions.document_id, chunks.section_id,
       documents.source_ref,
       CASE json_type(revisions.metadata_json, '$.version')
         WHEN 'text' THEN json_extract(revisions.metadata_json, '$.version') END,
       chunks.span_start, chunks.span_end, revisions.original_digest
     FROM chunks
     JOIN revisions ON revisions.id = chunks.revision_id
     JOIN documents ON documents.id = revisions.document_id
     WHERE chunks.chunk_set_id = ?1 AND chunks.id = ?2";

/// The source text a chunk's span covers in its revision's original
/// Markdown, read from the artifact store and checked against the revision's
/// digest, with what identifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excerpt {
    /// The revision whose original Markdown holds it.
    pub revision_id: String,
    /// The document of that revision.
    pub document_id: String,
    /// The chunk's section, if it has one.
    pub section_id: Option<String>,
    /// Where the document comes from: its origin URL, or `corpus-path:` and
    /// its path when it has none.
    pub source_ref: String,
    /// The revision's `version` metadata, absent when it has none as text.
    pub version: Option<String>,
    /// The chunk's span: UTF-8 byte offsets into the original Markdown.
    pub span: Span,
    /// The SHA-256 of `text`. The chunk's own digest hashes its prepared
    /// input, context included, so it is never this one.
    pub digest: Digest,
    /// The bytes of the span, verbatim.
    pub text: String,
}

/// A chunk found in its chunk set, with what identifies it and the digest of
/// its revision's original Markdown.
struct Located {
    /// [`Excerpt::revision_id`].
    revision_id: String,
    /// [`Excerpt::document_id`].
    document_id: String,
    /// [`Excerpt::section_id`].
    section_id: Option<String>,
    /// [`Excerpt::source_ref`].
    source_ref: String,
    /// [`Excerpt::version`].
    version: Option<String>,
    /// [`Excerpt::span`].
    span: Span,
    /// The digest the revision recorded for its original Markdown.
    original: Digest,
}

impl Database {
    /// The excerpt of the chunk `chunk_id` of the chunk set `chunk_set_id`,
    /// if `scopes` covers the scope of its document's source: the bytes of its
    /// span in its revision's original Markdown, read from the artifact store
    /// and checked against the revision's `original_digest`. A chunk outside
    /// the set is refused as one that does not exist, so a refusal never
    /// reveals that it does.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownChunk`] when the chunk set holds no such chunk that
    /// `scopes` covers, [`Error::DigestMismatch`] when the stored original no longer matches
    /// the revision's digest, [`Error::SpanOutOfRange`] when the span reaches
    /// past its end, [`Error::SpanOffBoundary`] when the span starts or ends
    /// inside a character, and [`Error::Store`] when the database or the
    /// artifact store cannot be read, a missing original among them.
    pub fn resolve(
        &self,
        scopes: &ScopeSet,
        chunk_set_id: &str,
        chunk_id: &str,
    ) -> Result<Excerpt, Error> {
        let located = self
            .reader()?
            .query_row(
                &format!(
                    "{LOCATE} AND {}",
                    ScopeSet::source_condition("documents.collection_id", "documents.source_id", 3)
                ),
                params![chunk_set_id, chunk_id, scopes.parameter()],
                located,
            )
            .optional()?
            .ok_or_else(|| Error::UnknownChunk {
                chunk_set_id: chunk_set_id.to_owned(),
                chunk_id: chunk_id.to_owned(),
            })?;
        let original = self.get(&located.original).map_err(|error| match error {
            store::Error::Artifact(artifact::Error::Corrupt { expected, found }) => {
                Error::DigestMismatch {
                    revision_id: located.revision_id.clone(),
                    expected,
                    found,
                }
            }
            other => Error::Store(other),
        })?;
        let Some(bytes) = original.get(located.span.start..located.span.end) else {
            return Err(Error::SpanOutOfRange {
                revision_id: located.revision_id,
                span: located.span,
                length: original.len(),
            });
        };
        let Ok(text) = str::from_utf8(bytes) else {
            return Err(Error::SpanOffBoundary {
                revision_id: located.revision_id,
                span: located.span,
            });
        };
        Ok(Excerpt {
            revision_id: located.revision_id,
            document_id: located.document_id,
            section_id: located.section_id,
            source_ref: located.source_ref,
            version: located.version,
            span: located.span,
            digest: Digest::of(bytes),
            text: text.to_owned(),
        })
    }
}

/// The chunk a row of [`LOCATE`] holds.
fn located(row: &Row<'_>) -> rusqlite::Result<Located> {
    // The table keeps both offsets at zero or above.
    let offset = |index| {
        let value: i64 = row.get(index)?;
        usize::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
    };
    let original: String = row.get(7)?;
    Ok(Located {
        revision_id: row.get(0)?,
        document_id: row.get(1)?,
        section_id: row.get(2)?,
        source_ref: row.get(3)?,
        version: row.get(4)?,
        span: Span {
            start: offset(5)?,
            end: offset(6)?,
        },
        original: Digest::parse(&original).map_err(|invalid| {
            rusqlite::Error::FromSqlConversionFailure(7, Type::Text, Box::new(invalid))
        })?,
    })
}
