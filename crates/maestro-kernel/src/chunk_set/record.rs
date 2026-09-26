//! Chunk sets as the kernel records them, and the calls that begin, end and
//! read them.

use super::{error::Error, state::ChunkSetState};
use crate::{
    artifact::Digest,
    scope::ScopeSet,
    store::{Database, artifacts},
};
use rusqlite::{Connection, OptionalExtension as _, Row, Transaction, params, types::Type};

/// The columns [`chunk_set_row`] reads, in its order.
const COLUMNS: &str = "id, collection_id, chunk_profile, counter_contract_id, state, \
                       manifest_digest";

/// A chunk set to begin: the chunks of a collection's eligible revisions
/// under one chunk profile and one token counter, named by an id its caller
/// derives from all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewChunkSet<'a> {
    /// Its id.
    pub id: &'a str,
    /// The collection whose revisions it chunks.
    pub collection_id: &'a str,
    /// The chunker's profile, such as `mapped-structural-chunks/2`.
    pub chunk_profile: &'a str,
    /// The contract ID of the token counter that counts its chunks.
    pub counter_contract_id: &'a str,
}

/// A chunk set as the kernel records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSet {
    /// Its id.
    pub id: String,
    /// The collection whose revisions it chunks.
    pub collection_id: String,
    /// The chunker's profile.
    pub chunk_profile: String,
    /// The contract ID of the token counter that counts its chunks.
    pub counter_contract_id: String,
    /// Where it is in its lifecycle.
    pub state: ChunkSetState,
    /// The artifact that describes it once complete, which it pins.
    pub manifest_digest: Option<Digest>,
}

impl Database {
    /// Begins the chunk set `new`, building, and returns it; a chunk set
    /// recorded before with the same collection, profile and counter is
    /// returned as it is, whatever its state, so a rerun resumes one that is
    /// building and finds one that is complete or failed.
    ///
    /// # Errors
    ///
    /// [`Error::Conflict`] when its id is recorded for another collection,
    /// profile or counter, and [`Error::Store`] when its collection is not
    /// recorded or the database cannot record it.
    pub fn begin_chunk_set(&self, new: &NewChunkSet<'_>) -> Result<ChunkSet, Error> {
        self.write(|transaction| {
            if let Some(recorded) = find(transaction, None, new.id)? {
                return found_again(recorded, new);
            }
            let begun = transaction.query_row(
                &format!(
                    "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id,
                       state)
                     VALUES (?1, ?2, ?3, ?4, 'building')
                     RETURNING {COLUMNS}"
                ),
                params![
                    new.id,
                    new.collection_id,
                    new.chunk_profile,
                    new.counter_contract_id,
                ],
                chunk_set_row,
            )?;
            Ok(begun)
        })
    }

    /// Moves the chunk set `id` from building to complete, with the artifact
    /// `manifest` that describes it, which it pins in the same write.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownChunkSet`], [`Error::IllegalMove`] when it is not
    /// building, and [`Error::Store`] when the manifest is not stored or the
    /// database cannot record the move: the set stays building then.
    pub fn complete_chunk_set(&self, id: &str, manifest: &Digest) -> Result<(), Error> {
        self.write(|transaction| {
            legal_move(transaction, id, ChunkSetState::Complete)?;
            transaction.execute(
                "UPDATE chunk_sets SET state = 'complete', manifest_digest = ?2 WHERE id = ?1",
                params![id, manifest.as_str()],
            )?;
            artifacts::pin(transaction, manifest)?;
            Ok(())
        })
    }

    /// Moves the chunk set `id` from building to failed, which is final: it
    /// is never completed nor resumed.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownChunkSet`], [`Error::IllegalMove`] when it is not
    /// building, and [`Error::Store`] when the database cannot record the
    /// move.
    pub fn fail_chunk_set(&self, id: &str) -> Result<(), Error> {
        self.write(|transaction| {
            legal_move(transaction, id, ChunkSetState::Failed)?;
            transaction.execute("UPDATE chunk_sets SET state = 'failed' WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// The chunk set `id`, if it is recorded and `scopes` covers the scope of
    /// its collection, whatever its state.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn chunk_set(&self, scopes: &ScopeSet, id: &str) -> Result<Option<ChunkSet>, Error> {
        Ok(find(&self.reader()?, Some(scopes), id)?)
    }
}

/// `recorded`, the chunk set of the id of `new`, if it was begun for the
/// same collection, profile and counter.
///
/// # Errors
///
/// [`Error::Conflict`] otherwise.
fn found_again(recorded: ChunkSet, new: &NewChunkSet<'_>) -> Result<ChunkSet, Error> {
    let same = recorded.collection_id == new.collection_id
        && recorded.chunk_profile == new.chunk_profile
        && recorded.counter_contract_id == new.counter_contract_id;
    if same {
        Ok(recorded)
    } else {
        Err(Error::Conflict(new.id.to_owned()))
    }
}

/// Whether the chunk set `id`, as `transaction` records it, may move to `to`.
///
/// # Errors
///
/// [`Error::UnknownChunkSet`] and [`Error::IllegalMove`].
fn legal_move(transaction: &Transaction<'_>, id: &str, to: ChunkSetState) -> Result<(), Error> {
    let set = find(transaction, None, id)?.ok_or_else(|| Error::UnknownChunkSet(id.to_owned()))?;
    if set.state.may_move_to(to) {
        Ok(())
    } else {
        Err(Error::IllegalMove {
            chunk_set: id.to_owned(),
            from: set.state,
            to,
        })
    }
}

/// The chunk set `id` that `connection` records, if any and `scopes` covers
/// the scope of its collection; with no set, whatever its scope, as a write
/// checks a chunk set against the one recorded.
pub(super) fn find(
    connection: &Connection,
    scopes: Option<&ScopeSet>,
    id: &str,
) -> rusqlite::Result<Option<ChunkSet>> {
    connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM chunk_sets WHERE id = ?1 AND (?2 IS NULL OR {})",
                ScopeSet::collection_condition("chunk_sets.collection_id", 2)
            ),
            params![id, scopes.map(ScopeSet::parameter)],
            chunk_set_row,
        )
        .optional()
}

/// The chunk set of a row of [`COLUMNS`].
fn chunk_set_row(row: &Row<'_>) -> rusqlite::Result<ChunkSet> {
    let state: String = row.get(4)?;
    let state = ChunkSetState::named(&state).ok_or_else(|| {
        let unknown = format!("no chunk set state is named {state:?}");
        rusqlite::Error::FromSqlConversionFailure(4, Type::Text, unknown.into())
    })?;
    let manifest: Option<String> = row.get(5)?;
    let manifest_digest = manifest
        .map(|hex| Digest::parse(&hex))
        .transpose()
        .map_err(|invalid| {
            rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(invalid))
        })?;
    Ok(ChunkSet {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        chunk_profile: row.get(2)?,
        counter_contract_id: row.get(3)?,
        state,
        manifest_digest,
    })
}
