//! Shared literal identifier matching and query-whitespace normalization.

use super::{
    error::Error,
    read::{classify, controlled_reader, ready_projection},
    types::{ChunkHit, SearchRead},
};
use crate::{scope::ScopeSet, store::Database};
use rusqlite::{Transaction, TransactionBehavior, params};

/// Trims `text` and collapses each Unicode-whitespace run to one ASCII space,
/// preserving every other character exactly.
#[must_use]
pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `identifier` occurs literally in `text` as a complete atom.
///
/// Text whitespace is normalized as it is for query understanding. Matching
/// remains case-, accent-, and punctuation-sensitive. A final period is a
/// delimiter only at the end of the text or immediately before a delimiter.
#[must_use]
pub fn contains_identifier(text: &str, identifier: &str) -> bool {
    if identifier.is_empty() {
        return false;
    }

    let text = normalize_whitespace(text);
    let mut remaining = text.as_str();
    let mut previous = None;
    while let Some(relative_start) = remaining.find(identifier) {
        let (before, candidate) = remaining.split_at(relative_start);
        if left_boundary(before.chars().next_back().or(previous))
            && let Some(after_identifier) = candidate.strip_prefix(identifier)
            && right_boundary(after_identifier)
        {
            return true;
        }
        let Some(character) = candidate.chars().next() else {
            return false;
        };
        previous = Some(character);
        remaining = candidate.split_at(character.len_utf8()).1;
    }
    false
}

/// Whether the character before a candidate, if any, is a permitted edge.
fn left_boundary(previous: Option<char>) -> bool {
    previous.is_none_or(delimiter)
}

/// Whether the character after an identifier, if any, is a permitted edge.
fn right_boundary(after_identifier: &str) -> bool {
    match after_identifier.chars().next() {
        None => true,
        Some('.') => after_identifier[1..].chars().next().is_none_or(delimiter),
        Some(character) => delimiter(character),
    }
}

/// The punctuation and whitespace that terminate an identifier atom.
fn delimiter(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '(' | ')'
                | '['
                | ']'
                | '<'
                | '>'
                | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '"'
                | '\''
                | '`'
                | '|'
        )
}

impl Database {
    /// Finds scoped exact identifier matches in the ready pinned generation.
    ///
    /// FTS only identifies candidate rows; every returned input passes
    /// [`contains_identifier`]. If any identifier has no ASCII alphanumeric,
    /// the kernel streams the scoped generation's inputs instead of using
    /// FTS, whose tokenizer has no term for punctuation-only identifiers.
    ///
    /// # Errors
    ///
    /// [`Error::ProjectionMissing`] or [`Error::ProfileMismatch`] when the
    /// generation's identifier projection is unavailable; [`Error::Cancelled`]
    /// or [`Error::TimedOut`] when the controlled read stops; or [`Error::Store`].
    pub fn identifier_hits(
        &self,
        read: &SearchRead<'_>,
        identifiers: &[String],
        limit: usize,
    ) -> Result<Vec<ChunkHit>, Error> {
        read.control.check()?;
        if identifiers.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        if identifiers.len() > 64 {
            return Err(Error::TooLarge);
        }
        let streaming = identifiers
            .iter()
            .any(|identifier| !identifier.bytes().any(|byte| byte.is_ascii_alphanumeric()));
        // ponytail: punctuation-only identifiers scan scoped inputs; replace
        // only if measurements justify a second tokenizer.
        let match_expression = (!streaming).then(|| {
            identifiers
                .iter()
                .map(|identifier| format!("\"{}\"", identifier.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" OR ")
        });
        let mut connection = controlled_reader(self, read.control)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| classify(error, read.control))?;
        ready_projection(&transaction, read)?;
        let hits = exact_hits(
            &transaction,
            read,
            identifiers,
            limit,
            match_expression.as_deref(),
        )?;
        read.control.check()?;
        transaction
            .commit()
            .map_err(|error| classify(error, read.control))?;
        read.control.check()?;
        Ok(hits)
    }
}

/// Streams candidate inputs in chunk-ID order, accepting only literal matches.
fn exact_hits(
    transaction: &Transaction<'_>,
    read: &SearchRead<'_>,
    identifiers: &[String],
    limit: usize,
    match_expression: Option<&str>,
) -> Result<Vec<ChunkHit>, Error> {
    let scoped = ScopeSet::source_condition("documents.collection_id", "documents.source_id", 3);
    let fts_scope = ScopeSet::source_condition("documents.collection_id", "documents.source_id", 4);
    let sql = if match_expression.is_some() {
        format!(
            "SELECT chunks.id, chunks.revision_id, chunk_search_inputs.prepared_input
             FROM chunk_search_fts
             JOIN chunk_search_inputs ON chunk_search_inputs.rowid = chunk_search_fts.rowid
             JOIN chunks ON chunks.chunk_set_id = chunk_search_inputs.chunk_set_id
               AND chunks.id = chunk_search_inputs.chunk_id
             JOIN revisions ON revisions.id = chunks.revision_id
             JOIN documents ON documents.id = revisions.document_id
             JOIN quality_dispositions ON quality_dispositions.revision_id = revisions.id
             WHERE chunk_search_fts MATCH ?1 AND chunks.chunk_set_id = ?2
               AND documents.collection_id = ?3 AND {fts_scope} AND (?5 IS NULL OR
                 CASE json_type(revisions.metadata_json, '$.version')
                   WHEN 'text' THEN json_extract(revisions.metadata_json, '$.version') END = ?5)
               AND revisions.status <> 'failed'
               AND quality_dispositions.disposition IN ('accepted', 'accepted_with_warnings')
             ORDER BY bm25(chunk_search_fts), chunks.id"
        )
    } else {
        format!(
            "SELECT chunks.id, chunks.revision_id, chunk_search_inputs.prepared_input
             FROM chunk_search_inputs
             JOIN chunks ON chunks.chunk_set_id = chunk_search_inputs.chunk_set_id
               AND chunks.id = chunk_search_inputs.chunk_id
             JOIN revisions ON revisions.id = chunks.revision_id
             JOIN documents ON documents.id = revisions.document_id
             JOIN quality_dispositions ON quality_dispositions.revision_id = revisions.id
             WHERE chunks.chunk_set_id = ?1 AND documents.collection_id = ?2
               AND {scoped} AND (?4 IS NULL OR
                 CASE json_type(revisions.metadata_json, '$.version')
                   WHEN 'text' THEN json_extract(revisions.metadata_json, '$.version') END = ?4)
               AND revisions.status <> 'failed'
               AND quality_dispositions.disposition IN ('accepted', 'accepted_with_warnings')
             ORDER BY chunks.id"
        )
    };
    (|| {
        read.control.check()?;
        let mut statement = transaction
            .prepare(&sql)
            .map_err(|error| classify(error, read.control))?;
        let mut rows = match match_expression {
            Some(expression) => statement.query(params![
                expression,
                read.generation.chunk_set_id,
                read.generation.collection_id,
                read.scopes.parameter(),
                read.version,
            ]),
            None => statement.query(params![
                read.generation.chunk_set_id,
                read.generation.collection_id,
                read.scopes.parameter(),
                read.version,
            ]),
        }
        .map_err(|error| classify(error, read.control))?;
        let mut hits = Vec::new();
        while let Some(row) = rows.next().map_err(|error| classify(error, read.control))? {
            read.control.check()?;
            let chunk_id: String = row.get(0).map_err(|error| classify(error, read.control))?;
            let prepared_input: String =
                row.get(2).map_err(|error| classify(error, read.control))?;
            if !identifiers
                .iter()
                .any(|identifier| contains_identifier(&prepared_input, identifier))
            {
                continue;
            }
            hits.push(ChunkHit {
                chunk_id,
                revision_id: row.get(1).map_err(|error| classify(error, read.control))?,
            });
            if hits.len() == limit {
                break;
            }
        }
        read.control.check()?;
        Ok(hits)
    })()
}
