//! Atomic writes of exact search derivatives and readiness markers.

use super::{
    error::Error,
    types::{SearchInput, SearchMember, SearchProjection},
};
use crate::{artifact::Digest, scope::ScopeSet, store::Database};
use rusqlite::{OptionalExtension as _, Transaction, params};
use std::collections::{BTreeMap, BTreeSet};

impl Database {
    /// Records up to 64 exact prepared inputs and their identifier values for a
    /// chunk set, idempotently.
    ///
    /// Each input must have its chunk's recorded digest and owner-source scope.
    /// Identifier rows are written in this same short write transaction.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownOrInaccessible`] when a chunk is missing or out of
    /// scope; [`Error::InvalidInput`] when its digest differs;
    /// [`Error::InputConflict`] when a retry changes stored text;
    /// [`Error::TooLarge`] for more than 64 inputs; or [`Error::Store`].
    pub fn record_search_inputs(
        &self,
        scopes: &ScopeSet,
        chunk_set_id: &str,
        inputs: &[SearchInput],
    ) -> Result<(), Error> {
        if inputs.len() > 64 {
            return Err(Error::TooLarge);
        }
        self.write(|transaction| record_inputs(transaction, scopes, chunk_set_id, inputs))
    }

    /// Records a complete chunk-set member list once, allowing only an
    /// identical retry. The caller must cover the entire collection.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownOrInaccessible`] when the chunk set is missing or its
    /// collection is out of scope; [`Error::InvalidInput`] for malformed
    /// member relationships; [`Error::MembershipConflict`] for a changed
    /// retry; or [`Error::Store`].
    pub fn record_search_members(
        &self,
        scopes: &ScopeSet,
        chunk_set_id: &str,
        members: &[SearchMember],
    ) -> Result<(), Error> {
        self.write(|transaction| record_members(transaction, scopes, chunk_set_id, members))
    }

    /// Begins the search projection for a Building generation.
    ///
    /// Returns `true` only when it inserts a marker. An identical marker is
    /// left unchanged and returns `false`.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownOrInaccessible`] when the generation is missing or
    /// outside the caller's collection scope; [`Error::InvalidInput`] when
    /// it is not Building; [`Error::ProfileMismatch`] for a changed retry;
    /// or [`Error::Store`].
    pub fn begin_generation_search(
        &self,
        scopes: &ScopeSet,
        generation_id: i64,
        profile: &str,
    ) -> Result<bool, Error> {
        self.write(|transaction| begin_search(transaction, scopes, generation_id, profile))
    }

    /// Returns the search marker when the caller covers the generation's
    /// collection, including a marker which is not ready yet.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn generation_search(
        &self,
        scopes: &ScopeSet,
        generation_id: i64,
    ) -> Result<Option<SearchProjection>, Error> {
        let projection = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT generation_search.identifier_profile, generation_search.ready
                     FROM generation_search JOIN generations
                       ON generations.id = generation_search.generation_id
                     WHERE generations.id = ?1 AND {}",
                    ScopeSet::collection_condition("generations.collection_id", 2)
                ),
                params![generation_id, scopes.parameter()],
                |row| {
                    Ok(SearchProjection {
                        identifier_profile: row.get(0)?,
                        ready: row.get(1)?,
                    })
                },
            )
            .optional()?;
        Ok(projection)
    }

    /// Marks a checked projection ready, idempotently, while its generation
    /// remains Building.
    ///
    /// The publisher calls this only after verifying identifier membership,
    /// chunk-set members, payload values and keyword indexes.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownOrInaccessible`] when the generation is missing or
    /// outside the caller's collection scope; [`Error::ProjectionMissing`]
    /// when no marker exists; [`Error::InvalidInput`] when it is not Building;
    /// or [`Error::Store`].
    pub fn complete_generation_search(
        &self,
        scopes: &ScopeSet,
        generation_id: i64,
    ) -> Result<(), Error> {
        self.write(|transaction| complete_search(transaction, scopes, generation_id))
    }
}

/// Checks the digest and idempotently inserts one input and its identifiers.
fn record_inputs(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    chunk_set_id: &str,
    inputs: &[SearchInput],
) -> Result<(), Error> {
    for input in inputs {
        record_input(transaction, scopes, chunk_set_id, input)?;
    }
    Ok(())
}

/// Checks and records one exact prepared input and its identifier set.
fn record_input(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    chunk_set_id: &str,
    input: &SearchInput,
) -> Result<(), Error> {
    let digest = transaction
        .query_row(
            &format!(
                "SELECT chunks.digest FROM chunks
                 JOIN revisions ON revisions.id = chunks.revision_id
                 JOIN documents ON documents.id = revisions.document_id
                 WHERE chunks.chunk_set_id = ?1 AND chunks.id = ?2 AND {}",
                ScopeSet::source_condition("documents.collection_id", "documents.source_id", 3)
            ),
            params![chunk_set_id, input.chunk_id, scopes.parameter()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(Error::UnknownOrInaccessible)?;
    let expected = Digest::of(input.prepared_input.as_bytes());
    let digest_matches = digest == expected.as_str();
    let previous = transaction
        .query_row(
            "SELECT prepared_input FROM chunk_search_inputs
             WHERE chunk_set_id = ?1 AND chunk_id = ?2",
            params![chunk_set_id, input.chunk_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match previous {
        Some(previous) if digest_matches && previous == input.prepared_input => {
            record_identifiers(transaction, chunk_set_id, input)?;
            Ok(())
        }
        Some(_) if digest_matches => Err(Error::InputConflict),
        None if digest_matches => {
            transaction.execute(
                "INSERT INTO chunk_search_inputs
                 (chunk_set_id, chunk_id, prepared_input) VALUES (?1, ?2, ?3)",
                params![chunk_set_id, input.chunk_id, input.prepared_input],
            )?;
            record_identifiers(transaction, chunk_set_id, input)
        }
        Some(_) | None => Err(Error::InvalidInput(
            "prepared input digest differs from its chunk".to_owned(),
        )),
    }
}

/// Records the immutable exact identifier set, or verifies an identical retry.
fn record_identifiers(
    transaction: &Transaction<'_>,
    chunk_set_id: &str,
    input: &SearchInput,
) -> Result<(), Error> {
    let requested: Vec<String> = input
        .identifiers
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let existing = {
        let mut statement = transaction.prepare(
            "SELECT identifier FROM chunk_search_identifiers
             WHERE chunk_set_id = ?1 AND chunk_id = ?2 ORDER BY identifier",
        )?;
        statement
            .query_map(params![chunk_set_id, input.chunk_id], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    if existing == requested {
        return Ok(());
    }
    if !existing.is_empty() {
        return Err(Error::InputConflict);
    }
    for identifier in requested {
        transaction.execute(
            "INSERT INTO chunk_search_identifiers
             (chunk_set_id, identifier, chunk_id) VALUES (?1, ?2, ?3)",
            params![chunk_set_id, identifier, input.chunk_id],
        )?;
    }
    Ok(())
}

/// Validates and records one complete chunk-set membership list.
fn record_members(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    chunk_set_id: &str,
    members: &[SearchMember],
) -> Result<(), Error> {
    let collection = transaction
        .query_row(
            &format!(
                "SELECT collection_id FROM chunk_sets
                 WHERE id = ?1 AND {}",
                ScopeSet::collection_condition("chunk_sets.collection_id", 2)
            ),
            params![chunk_set_id, scopes.parameter()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(Error::UnknownOrInaccessible)?;
    let by_revision = validate_members(transaction, chunk_set_id, &collection, members)?;
    let existing = read_members(transaction, chunk_set_id)?;
    let requested: Vec<_> = by_revision.into_iter().collect();
    if existing == requested {
        return Ok(());
    }
    if !existing.is_empty() {
        return Err(Error::MembershipConflict);
    }
    for (revision, representative) in requested {
        transaction.execute(
            "INSERT INTO chunk_set_members
             (chunk_set_id, revision_id, representative_revision_id)
             VALUES (?1, ?2, ?3)",
            params![chunk_set_id, revision, representative],
        )?;
    }
    Ok(())
}

/// Checks collection, document uniqueness, representative membership and chunks.
fn validate_members(
    transaction: &Transaction<'_>,
    chunk_set_id: &str,
    collection: &str,
    members: &[SearchMember],
) -> Result<BTreeMap<String, String>, Error> {
    let mut by_revision = BTreeMap::new();
    let mut documents = BTreeSet::new();
    for member in members {
        validate_member(
            transaction,
            collection,
            member,
            &mut documents,
            &mut by_revision,
        )?;
    }
    let representatives: BTreeSet<_> = by_revision.values().collect();
    for representative in representatives {
        validate_representative(transaction, chunk_set_id, &by_revision, representative)?;
    }
    validate_chunk_owners(transaction, chunk_set_id, &by_revision)?;
    Ok(by_revision)
}

/// Checks one member's collection, document uniqueness and revision uniqueness.
fn validate_member(
    transaction: &Transaction<'_>,
    collection: &str,
    member: &SearchMember,
    documents: &mut BTreeSet<String>,
    by_revision: &mut BTreeMap<String, String>,
) -> Result<(), Error> {
    let record = transaction
        .query_row(
            "SELECT documents.id, documents.collection_id
             FROM revisions JOIN documents ON documents.id = revisions.document_id
             WHERE revisions.id = ?1",
            [&member.revision_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| invalid_member("member revision is missing"))?;
    if record.1 != collection {
        return Err(invalid_member("member revision is from another collection"));
    }
    if !documents.insert(record.0) {
        return Err(invalid_member(
            "member list contains more than one revision per document",
        ));
    }
    if by_revision
        .insert(
            member.revision_id.clone(),
            member.representative_revision_id.clone(),
        )
        .is_some()
    {
        return Err(invalid_member("member revision is repeated"));
    }
    Ok(())
}

/// Checks one representative is a self-mapped member and owns set chunks.
fn validate_representative(
    transaction: &Transaction<'_>,
    chunk_set_id: &str,
    members: &BTreeMap<String, String>,
    representative: &String,
) -> Result<(), Error> {
    if members.get(representative) != Some(representative) {
        return Err(invalid_member(
            "representative revision is not a self-mapped member",
        ));
    }
    let owns_chunks: bool = transaction.query_row(
        "SELECT EXISTS (SELECT 1 FROM chunks
         WHERE chunk_set_id = ?1 AND revision_id = ?2)",
        params![chunk_set_id, representative],
        |row| row.get(0),
    )?;
    if !owns_chunks {
        return Err(invalid_member(
            "representative revision has no chunks in the set",
        ));
    }
    Ok(())
}

/// Ensures every chunk owner appears as a self-mapped member.
fn validate_chunk_owners(
    transaction: &Transaction<'_>,
    chunk_set_id: &str,
    members: &BTreeMap<String, String>,
) -> Result<(), Error> {
    let mut statement = transaction.prepare(
        "SELECT DISTINCT revision_id FROM chunks
         WHERE chunk_set_id = ?1 ORDER BY revision_id",
    )?;
    for owner in statement.query_map([chunk_set_id], |row| row.get::<_, String>(0))? {
        let owner = owner?;
        if members.get(&owner) != Some(&owner) {
            return Err(invalid_member(
                "a chunk-owning revision is absent from the member list",
            ));
        }
    }
    Ok(())
}

/// Begins a generation's immutable identifier-profile marker.
fn begin_search(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    generation_id: i64,
    profile: &str,
) -> Result<bool, Error> {
    let state = generation_state(transaction, scopes, generation_id)?;
    if state != "building" {
        return Err(Error::InvalidInput(
            "search projection requires a Building generation".to_owned(),
        ));
    }
    let existing = transaction
        .query_row(
            "SELECT identifier_profile FROM generation_search WHERE generation_id = ?1",
            [generation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match existing {
        Some(found) if found == profile => Ok(false),
        Some(found) => Err(Error::ProfileMismatch {
            expected: profile.to_owned(),
            found,
        }),
        None => {
            transaction.execute(
                "INSERT INTO generation_search (generation_id, identifier_profile)
                 VALUES (?1, ?2)",
                params![generation_id, profile],
            )?;
            Ok(true)
        }
    }
}

/// Marks a previously created marker ready, idempotently.
fn complete_search(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    generation_id: i64,
) -> Result<(), Error> {
    let state = generation_state(transaction, scopes, generation_id)?;
    if state != "building" {
        return Err(Error::InvalidInput(
            "search projection requires a Building generation".to_owned(),
        ));
    }
    let ready = transaction
        .query_row(
            "SELECT ready FROM generation_search WHERE generation_id = ?1",
            [generation_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .ok_or(Error::ProjectionMissing)?;
    if !ready {
        transaction.execute(
            "UPDATE generation_search SET ready = 1 WHERE generation_id = ?1",
            [generation_id],
        )?;
    }
    Ok(())
}

/// The members the chunk set records in revision order.
fn read_members(
    transaction: &Transaction<'_>,
    chunk_set_id: &str,
) -> Result<Vec<(String, String)>, Error> {
    let mut statement = transaction.prepare(
        "SELECT revision_id, representative_revision_id FROM chunk_set_members
         WHERE chunk_set_id = ?1 ORDER BY revision_id",
    )?;
    Ok(statement
        .query_map([chunk_set_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?)
}

/// The state of a generation whose collection is in `scopes`.
fn generation_state(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    generation_id: i64,
) -> Result<String, Error> {
    transaction
        .query_row(
            &format!(
                "SELECT state FROM generations WHERE id = ?1 AND {}",
                ScopeSet::collection_condition("generations.collection_id", 2)
            ),
            params![generation_id, scopes.parameter()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(Error::UnknownOrInaccessible)
}

/// A malformed member relationship, never a JSON or store parsing failure.
fn invalid_member(reason: &str) -> Error {
    Error::InvalidInput(reason.to_owned())
}
