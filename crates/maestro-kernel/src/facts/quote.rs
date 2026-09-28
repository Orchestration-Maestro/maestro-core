//! Verifying a claim's support from the authority: the revision is one the
//! caller may read in the claim's collection, and the exact bytes of the
//! span, read from its original Markdown and checked against its digest,
//! hash to the quote digest. Nothing here reads canonical blocks: which
//! block holds the span is for the caller to have located.

use super::error::Error;
use crate::{
    artifact::{self, Digest},
    evidence::Span,
    scope::ScopeSet,
    store::{self, Database},
};
use rusqlite::{Connection, OptionalExtension as _, params, types::Type};
use std::str;

/// The original Markdown of a revision a claim may quote.
pub(super) struct Original {
    /// The revision.
    revision_id: String,
    /// Its bytes, checked against its recorded digest.
    bytes: Vec<u8>,
}

impl Database {
    /// The original Markdown of the revision `revision_id`, if it belongs to
    /// the collection `collection_id` and `scopes` covers its document's
    /// source; refused as unknown otherwise, so a refusal never reveals one.
    /// Eligibility is not checked here: it can change, so the write checks
    /// it with [`eligible`] in its own transaction.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownRevision`] when no such revision is visible,
    /// [`Error::DigestMismatch`] when its stored bytes no longer match its
    /// digest, and [`Error::Store`] when the database or the artifact store
    /// cannot be read, a missing original among them.
    pub(super) fn claim_original(
        &self,
        scopes: &ScopeSet,
        collection_id: &str,
        revision_id: &str,
    ) -> Result<Original, Error> {
        let digest = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT revisions.original_digest FROM revisions
                     JOIN documents ON documents.id = revisions.document_id
                     WHERE revisions.id = ?1 AND documents.collection_id = ?2 AND {}",
                    ScopeSet::source_condition("documents.collection_id", "documents.source_id", 3)
                ),
                params![revision_id, collection_id, scopes.parameter()],
                |row| {
                    let text: String = row.get(0)?;
                    Digest::parse(&text).map_err(|invalid| {
                        rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(invalid))
                    })
                },
            )
            .optional()?
            .ok_or_else(|| Error::UnknownRevision {
                revision_id: revision_id.to_owned(),
            })?;
        let bytes = self.get(&digest).map_err(|error| match error {
            store::Error::Artifact(artifact::Error::Corrupt { expected, found }) => {
                Error::DigestMismatch {
                    revision_id: revision_id.to_owned(),
                    expected,
                    found,
                }
            }
            other => Error::Store(other),
        })?;
        Ok(Original {
            revision_id: revision_id.to_owned(),
            bytes,
        })
    }
}

impl Original {
    /// Checks that `span` is within these bytes and on character boundaries,
    /// and that its bytes hash to `quote_digest`.
    ///
    /// # Errors
    ///
    /// [`Error::SpanOutOfRange`],
    /// [`Error::SpanOffBoundary`] and [`Error::QuoteMismatch`].
    pub(super) fn check_quote(&self, span: Span, quote_digest: &Digest) -> Result<(), Error> {
        let Some(bytes) = self.bytes.get(span.start..span.end) else {
            return Err(Error::SpanOutOfRange {
                revision_id: self.revision_id.clone(),
                span,
                length: self.bytes.len(),
            });
        };
        if str::from_utf8(bytes).is_err() {
            return Err(Error::SpanOffBoundary {
                revision_id: self.revision_id.clone(),
                span,
            });
        }
        let found = Digest::of(bytes);
        if found == *quote_digest {
            Ok(())
        } else {
            Err(Error::QuoteMismatch {
                revision_id: self.revision_id.clone(),
                span,
                expected: quote_digest.clone(),
                found,
            })
        }
    }
}

/// Whether the revision `revision_id` may support a claim now, by the
/// quality gate's rule: it is its document's latest revision in record
/// order, it is not failed, and its disposition is `accepted` or
/// `accepted_with_warnings`. An older revision never stands in for a latest
/// one that is held, failed or not decided yet.
///
/// # Errors
///
/// [`Error::IneligibleRevision`] when it may not, and [`Error::Store`] when
/// the database cannot be read.
pub(super) fn eligible(connection: &Connection, revision_id: &str) -> Result<(), Error> {
    let eligible = connection
        .query_row(
            "SELECT 1 FROM revisions
             JOIN quality_dispositions ON quality_dispositions.revision_id = revisions.id
             WHERE revisions.id = ?1 AND revisions.status <> 'failed'
               AND quality_dispositions.disposition IN ('accepted', 'accepted_with_warnings')
               AND NOT EXISTS (SELECT 1 FROM revisions AS later
                 WHERE later.document_id = revisions.document_id
                   AND later.rowid > revisions.rowid)",
            [revision_id],
            |_| Ok(()),
        )
        .optional()?;
    eligible.ok_or_else(|| Error::IneligibleRevision {
        revision_id: revision_id.to_owned(),
    })
}
