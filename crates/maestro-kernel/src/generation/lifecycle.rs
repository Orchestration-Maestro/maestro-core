//! Generations as the kernel records them, and the calls that create and move
//! them.

use super::{error::Error, state::GenerationState};
use crate::{scope::ScopeSet, store::Database};
use rusqlite::{Connection, OptionalExtension as _, Row, Transaction, params, types::Type};

/// The columns [`generation_row`] reads, in its order.
const COLUMNS: &str = "id, collection_id, chunk_set_id, embedding_profile, sparse_profile, \
                       state, point_count, published_at";

/// A generation to create: the chunk set of a collection it is built from,
/// and the profiles that represent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGeneration {
    /// The collection it is a projection of.
    pub collection_id: String,
    /// The chunk set of that collection it is built from.
    pub chunk_set_id: String,
    /// The embedding profile of its dense vectors.
    pub embedding_profile: String,
    /// The sparse profile its lexical vectors are analysed with, such as
    /// `bm25-en-fr/1`; a query is analysed with the same one.
    pub sparse_profile: String,
}

/// A generation as the kernel records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    /// Its id, never given to another generation.
    pub id: i64,
    /// The collection it is a projection of.
    pub collection_id: String,
    /// The chunk set it is built from.
    pub chunk_set_id: String,
    /// The embedding profile of its dense vectors.
    pub embedding_profile: String,
    /// The sparse profile of its lexical vectors.
    pub sparse_profile: String,
    /// Where it is in its lifecycle.
    pub state: GenerationState,
    /// How many points its verification counted, once verified.
    pub point_count: Option<u64>,
    /// When it was published, if it was, in RFC 3339 UTC.
    pub published_at: Option<String>,
}

impl Database {
    /// Creates a generation of `new`, `building`, and returns it.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when its collection is not recorded, its chunk set is
    /// not one of that collection's, or the database cannot record it.
    pub fn create_generation(&self, new: &NewGeneration) -> Result<Generation, Error> {
        self.write(|transaction| {
            let generation = transaction.query_row(
                &format!(
                    "INSERT INTO generations (collection_id, chunk_set_id, embedding_profile,
                       sparse_profile)
                     VALUES (?1, ?2, ?3, ?4)
                     RETURNING {COLUMNS}"
                ),
                params![
                    new.collection_id,
                    new.chunk_set_id,
                    new.embedding_profile,
                    new.sparse_profile,
                ],
                generation_row,
            )?;
            Ok(generation)
        })
    }

    /// Moves the generation `id` from `building` to `verified`, with the
    /// `point_count` its verification counted.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownGeneration`], [`Error::IllegalMove`] when it is not
    /// building, and [`Error::Store`] when the count does not fit SQLite's
    /// integers or the database cannot record the move.
    pub fn verify_generation(&self, id: i64, point_count: u64) -> Result<(), Error> {
        let point_count = i64::try_from(point_count)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.write(|transaction| {
            legal_move(transaction, id, GenerationState::Verified)?;
            transaction.execute(
                "UPDATE generations SET state = 'verified', point_count = ?2 WHERE id = ?1",
                params![id, point_count],
            )?;
            Ok(())
        })
    }

    /// Moves the generation `id` from `verified` to `published`, and in the
    /// same transaction retires the generation of its collection published
    /// before it, whose id it returns.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownGeneration`], [`Error::IllegalMove`] when it is not
    /// verified, and [`Error::Store`] when the database cannot record the
    /// move.
    pub fn publish_generation(&self, id: i64) -> Result<Option<i64>, Error> {
        self.write(|transaction| {
            let generation = legal_move(transaction, id, GenerationState::Published)?;
            // First, since the database holds one published generation per
            // collection at every statement.
            let retired = transaction
                .query_row(
                    "UPDATE generations SET state = 'retired'
                     WHERE collection_id = ?1 AND state = 'published'
                     RETURNING id",
                    [&generation.collection_id],
                    |row| row.get(0),
                )
                .optional()?;
            transaction.execute(
                "UPDATE generations
                 SET state = 'published', published_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1",
                [id],
            )?;
            Ok(retired)
        })
    }

    /// Moves the generation `id` from `published` to `retired`, which leaves
    /// its collection with no published generation.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownGeneration`], [`Error::IllegalMove`] when it is not
    /// published, and [`Error::Store`] when the database cannot record the
    /// move.
    pub fn retire_generation(&self, id: i64) -> Result<(), Error> {
        self.write(|transaction| move_to(transaction, id, GenerationState::Retired))
    }

    /// Moves the generation `id` from `building` or `verified` to `failed`,
    /// which is final: it is never published nor resumed, and the next
    /// generation of its collection is published as if it did not exist.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownGeneration`], [`Error::IllegalMove`] when it is
    /// published, retired or failed already, and [`Error::Store`] when the
    /// database cannot record the move.
    pub fn fail_generation(&self, id: i64) -> Result<(), Error> {
        self.write(|transaction| move_to(transaction, id, GenerationState::Failed))
    }

    /// The generation `id`, if it is recorded and `scopes` covers the scope
    /// of its collection, whatever its state: a revocation applies to a
    /// retired generation too.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn generation(&self, scopes: &ScopeSet, id: i64) -> Result<Option<Generation>, Error> {
        Ok(find(&self.reader()?, Some(scopes), id)?)
    }

    /// The published generation of the collection `collection_id`, if it has
    /// one and `scopes` covers the scope of the collection.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn published_generation(
        &self,
        scopes: &ScopeSet,
        collection_id: &str,
    ) -> Result<Option<Generation>, Error> {
        let published = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM generations
                     WHERE collection_id = ?1 AND state = 'published' AND {}",
                    ScopeSet::collection_condition("generations.collection_id", 2)
                ),
                params![collection_id, scopes.parameter()],
                generation_row,
            )
            .optional()?;
        Ok(published)
    }

    /// Every generation of the collection `collection_id`, whatever its
    /// state, in the order they were created, if `scopes` covers the scope
    /// of the collection; none otherwise.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn generations(
        &self,
        scopes: &ScopeSet,
        collection_id: &str,
    ) -> Result<Vec<Generation>, Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT {COLUMNS} FROM generations WHERE collection_id = ?1 AND {} ORDER BY id",
            ScopeSet::collection_condition("generations.collection_id", 2)
        ))?;
        let generations = statement
            .query_map(params![collection_id, scopes.parameter()], generation_row)?
            .collect::<Result<_, _>>()?;
        Ok(generations)
    }
}

/// Moves the generation `id` to `to` inside `transaction`, when it may, and
/// changes nothing else.
///
/// # Errors
///
/// As [`legal_move`], and [`Error::Store`] when the move cannot be written.
fn move_to(transaction: &Transaction<'_>, id: i64, to: GenerationState) -> Result<(), Error> {
    legal_move(transaction, id, to)?;
    transaction.execute(
        "UPDATE generations SET state = ?2 WHERE id = ?1",
        params![id, to.as_str()],
    )?;
    Ok(())
}

/// The generation `id` as `transaction` records it, when it may move to `to`.
///
/// # Errors
///
/// [`Error::UnknownGeneration`] and [`Error::IllegalMove`], and
/// [`Error::Store`] when the database cannot be read.
fn legal_move(
    transaction: &Transaction<'_>,
    id: i64,
    to: GenerationState,
) -> Result<Generation, Error> {
    let generation = find(transaction, None, id)?.ok_or(Error::UnknownGeneration(id))?;
    if generation.state.may_move_to(to) {
        Ok(generation)
    } else {
        Err(Error::IllegalMove {
            generation: id,
            from: generation.state,
            to,
        })
    }
}

/// The generation `id` that `connection` records, if any and `scopes`
/// covers the scope of its collection; with no set, whatever its scope, as a
/// move checks the generation's state as recorded.
fn find(
    connection: &Connection,
    scopes: Option<&ScopeSet>,
    id: i64,
) -> rusqlite::Result<Option<Generation>> {
    connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM generations WHERE id = ?1 AND (?2 IS NULL OR {})",
                ScopeSet::collection_condition("generations.collection_id", 2)
            ),
            params![id, scopes.map(ScopeSet::parameter)],
            generation_row,
        )
        .optional()
}

/// The generation of a row of [`COLUMNS`].
fn generation_row(row: &Row<'_>) -> rusqlite::Result<Generation> {
    let state: String = row.get(5)?;
    let point_count: Option<i64> = row.get(6)?;
    Ok(Generation {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        chunk_set_id: row.get(2)?,
        embedding_profile: row.get(3)?,
        sparse_profile: row.get(4)?,
        state: GenerationState::named(&state).ok_or_else(|| {
            let unknown = format!("no generation state is named {state:?}");
            rusqlite::Error::FromSqlConversionFailure(5, Type::Text, unknown.into())
        })?,
        point_count: point_count
            .map(|count| {
                u64::try_from(count).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, count))
            })
            .transpose()?,
        published_at: row.get(7)?,
    })
}
