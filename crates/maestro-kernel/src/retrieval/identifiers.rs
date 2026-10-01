//! Shared literal identifier matching and query-whitespace normalization.

use super::{
    error::Error,
    read::{classify, controlled_reader, ready_projection},
    types::{ChunkHit, IdentifierSearchResult, SearchRead},
};
use crate::{scope::ScopeSet, store::Database};
use rusqlite::{Transaction, TransactionBehavior, params};
use std::collections::{BTreeMap, HashSet};

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
    /// Identifiers matching more chunks than the requested hit limit are
    /// skipped using the exact membership index. Counting stops one match
    /// beyond that limit because the route cannot rank a larger set.
    ///
    /// # Errors
    ///
    /// [`Error::ProjectionMissing`] or [`Error::ProfileMismatch`] when the
    /// generation's identifier projection is unavailable; [`Error::Cancelled`]
    /// or [`Error::TimedOut`] when the controlled read stops; [`Error::TooLarge`]
    /// when the request exceeds its identifier or SQLite limit; or [`Error::Store`].
    pub fn identifier_hits(
        &self,
        read: &SearchRead<'_>,
        identifiers: &[String],
        limit: usize,
    ) -> Result<IdentifierSearchResult, Error> {
        read.control.check()?;
        if identifiers.is_empty() || limit == 0 {
            return Ok(IdentifierSearchResult::default());
        }
        if identifiers.len() > 64 {
            return Err(Error::TooLarge);
        }
        let sqlite_limit = i64::try_from(limit).map_err(|_| Error::TooLarge)?;
        let mut connection = controlled_reader(self, read.control)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| classify(error, read.control))?;
        ready_projection(&transaction, read)?;

        let mut seen = HashSet::new();
        let requested_identifiers = identifiers
            .iter()
            .filter(|identifier| seen.insert(identifier.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let common =
            too_common_identifiers(&transaction, read, &requested_identifiers, sqlite_limit)?;
        let (too_common, active_identifiers): (Vec<_>, Vec<_>) = requested_identifiers
            .into_iter()
            .partition(|identifier| common.contains(identifier));
        let hits = if active_identifiers.is_empty() {
            Vec::new()
        } else {
            indexed_hits(&transaction, read, &active_identifiers, sqlite_limit)?
        };
        read.control.check()?;
        transaction
            .commit()
            .map_err(|error| classify(error, read.control))?;
        read.control.check()?;
        Ok(IdentifierSearchResult { hits, too_common })
    }
}

/// Counts each requested identifier only through one beyond the fetch limit.
fn too_common_identifiers(
    transaction: &Transaction<'_>,
    read: &SearchRead<'_>,
    identifiers: &[String],
    fetch_limit: i64,
) -> Result<HashSet<String>, Error> {
    let encoded_identifiers = serde_json::to_string(identifiers)
        .map_err(|_| Error::InvalidInput("identifier values could not be encoded".to_owned()))?;
    read.control.check()?;
    let mut statement = transaction
        .prepare(
            "SELECT requested.value,
                    (SELECT count(*) FROM (
                       SELECT 1 FROM chunk_search_identifiers
                       WHERE chunk_set_id = ?2 AND identifier = requested.value
                       LIMIT ?3
                     ))
             FROM json_each(?1) AS requested",
        )
        .map_err(|error| classify(error, read.control))?;
    let mut rows = statement
        .query(params![
            encoded_identifiers,
            read.generation.chunk_set_id,
            fetch_limit.saturating_add(1),
        ])
        .map_err(|error| classify(error, read.control))?;
    let mut common = HashSet::new();
    while let Some(row) = rows.next().map_err(|error| classify(error, read.control))? {
        read.control.check()?;
        let identifier: String = row.get(0).map_err(|error| classify(error, read.control))?;
        let count: i64 = row.get(1).map_err(|error| classify(error, read.control))?;
        if count > fetch_limit {
            common.insert(identifier);
        }
    }
    read.control.check()?;
    Ok(common)
}

/// Reads each identifier's scoped, eligible top hits and merges them by chunk ID.
pub(super) fn indexed_hits(
    transaction: &Transaction<'_>,
    read: &SearchRead<'_>,
    identifiers: &[String],
    limit: i64,
) -> Result<Vec<ChunkHit>, Error> {
    let limit_size = usize::try_from(limit).map_err(|_| Error::TooLarge)?;
    let scoped = ScopeSet::source_condition("documents.collection_id", "documents.source_id", 4);
    // Keep the composite-key order so each identifier yields a sorted, bounded stream.
    let sql = format!(
        "SELECT indexed.chunk_id, chunks.revision_id
         FROM chunk_search_identifiers AS indexed
         CROSS JOIN chunks
         CROSS JOIN revisions
         CROSS JOIN documents
         CROSS JOIN quality_dispositions
         WHERE indexed.chunk_set_id = ?1 AND indexed.identifier = ?2
           AND chunks.chunk_set_id = indexed.chunk_set_id AND chunks.id = indexed.chunk_id
           AND revisions.id = chunks.revision_id AND documents.id = revisions.document_id
           AND quality_dispositions.revision_id = revisions.id
           AND documents.collection_id = ?3 AND {scoped}
           AND (?5 IS NULL OR CASE json_type(revisions.metadata_json, '$.version')
             WHEN 'text' THEN json_extract(revisions.metadata_json, '$.version') END = ?5)
           AND revisions.status <> 'failed'
           AND quality_dispositions.disposition IN ('accepted', 'accepted_with_warnings')
         ORDER BY indexed.chunk_id LIMIT ?6"
    );
    read.control.check()?;
    let mut statement = transaction
        .prepare(&sql)
        .map_err(|error| classify(error, read.control))?;
    let scopes = read.scopes.parameter();
    let mut hits = BTreeMap::new();
    for identifier in identifiers {
        read.control.check()?;
        let mut rows = statement
            .query(params![
                read.generation.chunk_set_id,
                identifier,
                read.generation.collection_id,
                scopes,
                read.version,
                limit,
            ])
            .map_err(|error| classify(error, read.control))?;
        while let Some(row) = rows.next().map_err(|error| classify(error, read.control))? {
            read.control.check()?;
            let chunk_id: String = row.get(0).map_err(|error| classify(error, read.control))?;
            let revision_id: String = row.get(1).map_err(|error| classify(error, read.control))?;
            hits.entry(chunk_id.clone()).or_insert(revision_id);
            if hits.len() > limit_size {
                hits.pop_last();
            }
            if hits.len() == limit_size
                && hits
                    .last_key_value()
                    .is_some_and(|(last_chunk_id, _)| chunk_id >= *last_chunk_id)
            {
                break;
            }
        }
    }
    read.control.check()?;
    Ok(hits
        .into_iter()
        .map(|(chunk_id, revision_id)| ChunkHit {
            chunk_id,
            revision_id,
        })
        .collect())
}
