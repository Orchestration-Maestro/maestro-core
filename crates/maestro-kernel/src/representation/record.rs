//! Representation set lifecycle, revision shards and generation binding.
use super::{
    error::{Error, require},
    types::{RepresentationKey, RepresentationSetSpec},
    validation::{complete, load, validate_shard},
};
use crate::{
    artifact::Digest,
    scope::ScopeSet,
    store::{Database, artifacts},
};
use rusqlite::{OptionalExtension as _, params};

impl Database {
    /// Begins an immutable representation set without loading or writing artifacts.
    ///
    /// # Errors
    /// Unauthorized collection, conflicting identity or database failure.
    pub fn begin_representation_set(
        &self,
        scopes: &ScopeSet,
        spec: &RepresentationSetSpec,
    ) -> Result<(), Error> {
        self.write(|tx| {
            let authorized = tx
                .prepare(&format!(
                    "SELECT 1 FROM collections WHERE id=?1 AND {}",
                    ScopeSet::collection_condition("collections.id", 2)
                ))?
                .exists(params![spec.collection_id, scopes.parameter()])?;
            require(authorized, "unauthorized representation collection")?;
            let previous: Option<(String, String, String, String)> = tx
                .query_row(
                    "SELECT profile_digest,embedding_profile,sparse_profile,layout
                 FROM representation_sets
                 WHERE collection_id=?1 AND chunk_set_id=?2 AND id=?3",
                    params![spec.collection_id, spec.chunk_set_id, spec.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            if let Some((profile, dense, sparse, layout)) = previous {
                return (profile == spec.profile_digest.as_str()
                    && dense == spec.layout.embedding_profile
                    && sparse == spec.layout.sparse_profile
                    && layout == spec.layout.version)
                    .then_some(())
                    .ok_or(Error::Conflict);
            }
            tx.execute(
                "INSERT INTO representation_sets VALUES (?1,?2,?3,?4,?5,?6,?7,'building')",
                params![
                    spec.collection_id,
                    spec.chunk_set_id,
                    spec.id,
                    spec.profile_digest.as_str(),
                    spec.layout.embedding_profile,
                    spec.layout.sparse_profile,
                    spec.layout.version
                ],
            )?;
            Ok(())
        })
    }

    /// Validates, records and pins exactly one revision shard.
    ///
    /// # Errors
    /// Unauthorized/missing graph, invalid or conflicting shard, or storage failure.
    pub fn record_representation_revision(
        &self,
        scopes: &ScopeSet,
        key: &RepresentationKey<'_>,
        digest: &Digest,
    ) -> Result<(), Error> {
        let set = self
            .representation_set(scopes, key)?
            .ok_or(Error::NotFound)?;
        require(
            set.state == super::types::RepresentationState::Building,
            "representation set is not building",
        )?;
        let shard = load(self, digest)?;
        validate_shard(self, scopes, key, &shard)?;
        self.write(|tx| {
            let set = tx
                .query_row(
                    "SELECT state,profile_digest FROM representation_sets
                 WHERE collection_id=?1 AND chunk_set_id=?2 AND id=?3",
                    params![key.collection_id, key.chunk_set_id, key.id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let Some((state, profile)) = set else {
                return Err(Error::NotFound);
            };
            require(state == "building", "representation set is not building")?;
            let graph_digest: Option<String> = tx
                .query_row(
                    "SELECT graph_digest FROM revision_unit_graphs
                 WHERE collection_id=?1 AND chunk_set_id=?2 AND revision_id=?3",
                    params![key.collection_id, key.chunk_set_id, shard.revision_id],
                    |row| row.get(0),
                )
                .optional()?;
            require(
                graph_digest.as_deref() == Some(shard.graph_digest.as_str()),
                "shard graph digest differs from record",
            )?;
            let graph_profile: String = tx.query_row(
                "SELECT p.profile_digest FROM chunk_set_profiles AS p
                 WHERE p.collection_id=?1 AND p.chunk_set_id=?2",
                params![key.collection_id, key.chunk_set_id],
                |row| row.get(0),
            )?;
            require(
                profile == graph_profile,
                "representation profile differs from graph",
            )?;
            let previous: Option<String> = tx
                .query_row(
                    "SELECT shard_digest FROM representation_revisions
                 WHERE collection_id=?1 AND chunk_set_id=?2
                   AND representation_set_id=?3 AND revision_id=?4",
                    params![
                        key.collection_id,
                        key.chunk_set_id,
                        key.id,
                        shard.revision_id
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(previous) = previous {
                return (previous == digest.as_str())
                    .then_some(())
                    .ok_or(Error::Conflict);
            }
            tx.execute(
                "INSERT INTO representation_revisions VALUES (?1,?2,?3,?4,?5)",
                params![
                    key.collection_id,
                    key.chunk_set_id,
                    key.id,
                    shard.revision_id,
                    digest.as_str()
                ],
            )?;
            artifacts::pin(tx, digest)?;
            Ok(())
        })
    }

    /// Completes only a frozen chunk set with one recorded shard per graph revision.
    ///
    /// # Errors
    /// Missing/unauthorized set, missing shards, mutable chunk set or illegal transition.
    pub fn complete_representation_set(
        &self,
        scopes: &ScopeSet,
        key: &RepresentationKey<'_>,
    ) -> Result<(), Error> {
        self.representation_set(scopes, key)?
            .ok_or(Error::NotFound)?;
        complete(self, scopes, key)?;
        transition(self, scopes, key, "complete")
    }

    /// Marks a building representation failed, permanently.
    ///
    /// # Errors
    /// Unauthorized/unknown record or illegal state transition.
    pub fn fail_representation_set(
        &self,
        scopes: &ScopeSet,
        key: &RepresentationKey<'_>,
    ) -> Result<(), Error> {
        transition(self, scopes, key, "failed")
    }

    /// Binds a matching complete representation to a building generation once.
    ///
    /// # Errors
    /// Unauthorized/unknown representation, duplicate or mismatched binding.
    pub fn bind_generation_representation(
        &self,
        scopes: &ScopeSet,
        generation: i64,
        key: &RepresentationKey<'_>,
    ) -> Result<(), Error> {
        self.representation_set(scopes, key)?
            .ok_or(Error::NotFound)?;
        self.write(|tx| {
            let authorized = tx
                .prepare(&format!(
                    "SELECT 1 FROM collections WHERE id=?1 AND {}",
                    ScopeSet::collection_condition("collections.id", 2)
                ))?
                .exists(params![key.collection_id, scopes.parameter()])?;
            require(authorized, "unauthorized representation collection")?;
            tx.execute(
                "INSERT INTO generation_representations VALUES (?1,?2,?3,?4)",
                params![generation, key.collection_id, key.chunk_set_id, key.id],
            )?;
            Ok(())
        })
    }
}

/// Applies one scoped terminal state transition.
fn transition(
    db: &Database,
    scopes: &ScopeSet,
    key: &RepresentationKey<'_>,
    state: &str,
) -> Result<(), Error> {
    db.write(|tx| {
        let changed = tx.execute(
            &format!(
                "UPDATE representation_sets SET state=?4
             WHERE collection_id=?1 AND chunk_set_id=?2 AND id=?3
               AND state='building' AND {}",
                ScopeSet::collection_condition("representation_sets.collection_id", 5)
            ),
            params![
                key.collection_id,
                key.chunk_set_id,
                key.id,
                state,
                scopes.parameter()
            ],
        )?;
        if changed == 1 {
            return Ok(());
        }
        let exists = tx
            .prepare(&format!(
                "SELECT 1 FROM representation_sets
             WHERE collection_id=?1 AND chunk_set_id=?2 AND id=?3 AND {}",
                ScopeSet::collection_condition("representation_sets.collection_id", 4)
            ))?
            .exists(params![
                key.collection_id,
                key.chunk_set_id,
                key.id,
                scopes.parameter()
            ])?;
        if exists {
            Err(Error::Conflict)
        } else {
            Err(Error::NotFound)
        }
    })
}
