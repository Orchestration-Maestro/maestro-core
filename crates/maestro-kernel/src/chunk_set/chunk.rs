//! Chunks: the passages of a revision in a chunk set, each pinning the
//! artifact of its exact prepared input, recorded a revision at a time.

use super::{error::Error, record::find, state::ChunkSetState};
use crate::{
    artifact::Digest,
    evidence::Span,
    scope::ScopeSet,
    store::{Database, artifacts},
};
use rusqlite::{Connection, Row, Transaction, params, types::Type};
use std::num::TryFromIntError;

/// The columns [`chunk_row`] reads, in its order.
const COLUMNS: &str = "chunks.id, chunks.revision_id, chunks.section_id, chunks.digest, \
                       chunks.token_count, chunks.span_start, chunks.span_end";

/// A chunk: one passage of a revision, as the chunker prepared and counted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Its id, the chunker's, which unchanged content keeps from one chunk
    /// set to the next; unique in its chunk set.
    pub id: String,
    /// The revision it is a passage of.
    pub revision_id: String,
    /// The section that owns it, if any.
    pub section_id: Option<String>,
    /// The artifact of its exact prepared input, context included, which its
    /// chunk set pins: what the embedder reads is what was counted.
    pub digest: Digest,
    /// The tokens of its prepared input, as the chunk set's counter counts
    /// them.
    pub token_count: u64,
    /// The bytes of its revision's original Markdown it covers.
    pub span: Span,
}

impl Database {
    /// Records `chunks`, every chunk of the revision `revision_id`, in the
    /// building chunk set `chunk_set_id`, in one write that pins the artifact
    /// of each chunk's prepared input, once per chunk. The same chunks
    /// recorded before change nothing.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownChunkSet`]; [`Error::NotBuilding`] when the set is
    /// complete or failed; [`Error::ChunksConflict`] when a chunk is another
    /// revision's, or the set holds other chunks of the revision; and
    /// [`Error::Store`] when the revision is not of the set's collection, a
    /// prepared input is not stored, or the database cannot record them.
    /// Nothing is recorded then.
    pub fn record_chunks(
        &self,
        chunk_set_id: &str,
        revision_id: &str,
        chunks: &[Chunk],
    ) -> Result<(), Error> {
        self.write(|transaction| record(transaction, chunk_set_id, revision_id, chunks))
    }

    /// Every chunk of the chunk set `chunk_set_id` whose revision's document
    /// has a source `scopes` covers, in the order they were recorded,
    /// whatever the set's state: a reader that wants a complete set checks
    /// its state first.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn chunks(&self, scopes: &ScopeSet, chunk_set_id: &str) -> Result<Vec<Chunk>, Error> {
        Ok(read(
            &self.reader()?,
            &format!(
                "chunks.chunk_set_id = ?1 AND {}",
                ScopeSet::source_condition("documents.collection_id", "documents.source_id", 2)
            ),
            params![chunk_set_id, scopes.parameter()],
        )?)
    }
}

/// Records `chunks`, every chunk of the revision `revision_id`, in the
/// building chunk set `chunk_set_id`, inside `transaction`: as
/// [`Database::record_chunks`].
fn record(
    transaction: &Transaction<'_>,
    chunk_set_id: &str,
    revision_id: &str,
    chunks: &[Chunk],
) -> Result<(), Error> {
    let set = find(transaction, None, chunk_set_id)?
        .ok_or_else(|| Error::UnknownChunkSet(chunk_set_id.to_owned()))?;
    if set.state != ChunkSetState::Building {
        return Err(Error::NotBuilding {
            chunk_set: set.id,
            state: set.state,
        });
    }
    let conflict = || Error::ChunksConflict {
        chunk_set: chunk_set_id.to_owned(),
        revision: revision_id.to_owned(),
    };
    if chunks.iter().any(|chunk| chunk.revision_id != revision_id) {
        return Err(conflict());
    }
    let recorded = read(
        transaction,
        "chunks.chunk_set_id = ?1 AND chunks.revision_id = ?2",
        params![chunk_set_id, revision_id],
    )?;
    if !recorded.is_empty() {
        return (recorded == chunks).then_some(()).ok_or_else(conflict);
    }
    for chunk in chunks {
        transaction.execute(
            "INSERT INTO chunks (chunk_set_id, id, revision_id, section_id, digest, token_count,
               span_start, span_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                chunk_set_id,
                chunk.id,
                chunk.revision_id,
                chunk.section_id,
                chunk.digest.as_str(),
                integer(chunk.token_count)?,
                integer(chunk.span.start)?,
                integer(chunk.span.end)?,
            ],
        )?;
        artifacts::pin(transaction, &chunk.digest)?;
    }
    Ok(())
}

/// The chunks `connection` records that meet `condition` over `parameters`,
/// in the order they were recorded.
fn read(
    connection: &Connection,
    condition: &str,
    parameters: impl rusqlite::Params,
) -> rusqlite::Result<Vec<Chunk>> {
    // Each insert takes a rowid above every other, so rowid order is record
    // order.
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM chunks
         JOIN revisions ON revisions.id = chunks.revision_id
         JOIN documents ON documents.id = revisions.document_id
         WHERE {condition}
         ORDER BY chunks.rowid"
    ))?;
    statement.query_map(parameters, chunk_row)?.collect()
}

/// The chunk of a row of [`COLUMNS`].
fn chunk_row(row: &Row<'_>) -> rusqlite::Result<Chunk> {
    let digest: String = row.get(3)?;
    Ok(Chunk {
        id: row.get(0)?,
        revision_id: row.get(1)?,
        section_id: row.get(2)?,
        digest: Digest::parse(&digest).map_err(|invalid| {
            rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(invalid))
        })?,
        token_count: unsigned(row, 4)?,
        span: Span {
            start: unsigned(row, 5)?,
            end: unsigned(row, 6)?,
        },
    })
}

/// `value` as the SQLite integer its column holds.
fn integer<T: TryInto<i64, Error = TryFromIntError>>(value: T) -> rusqlite::Result<i64> {
    value
        .try_into()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

/// The integer of column `index`, which the table keeps at zero or above.
fn unsigned<T: TryFrom<i64>>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let value: i64 = row.get(index)?;
    T::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
