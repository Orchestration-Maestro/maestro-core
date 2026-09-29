//! Scoped representation metadata and individual revision-shard reads.
use super::{
    error::{Error, require},
    types::{
        RepresentationKey, RepresentationLayout, RepresentationSet, RepresentationShard,
        RepresentationState,
    },
    validation::load,
};
use crate::{artifact::Digest, scope::ScopeSet, store::Database};
use rusqlite::{OptionalExtension as _, params};

impl Database {
    /// Reads SQL representation identity, layout and state without loading shards.
    ///
    /// # Errors
    /// Invalid stored metadata or database failure.
    pub fn representation_set(
        &self,
        scopes: &ScopeSet,
        key: &RepresentationKey<'_>,
    ) -> Result<Option<RepresentationSet>, Error> {
        let reader = self.reader()?;
        let row = reader
            .query_row(
                &format!(
                    "SELECT profile_digest,embedding_profile,sparse_profile,layout,state
             FROM representation_sets WHERE collection_id=?1 AND chunk_set_id=?2 AND id=?3 AND {}",
                    ScopeSet::collection_condition("representation_sets.collection_id", 4)
                ),
                params![
                    key.collection_id,
                    key.chunk_set_id,
                    key.id,
                    scopes.parameter()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(profile, dense, sparse, version, state)| {
            Ok(RepresentationSet {
                collection_id: key.collection_id.to_owned(),
                chunk_set_id: key.chunk_set_id.to_owned(),
                id: key.id.to_owned(),
                profile_digest: Digest::parse(&profile)
                    .map_err(|_| Error::Invalid("stored profile digest"))?,
                layout: RepresentationLayout {
                    version,
                    embedding_profile: dense,
                    sparse_profile: sparse,
                },
                state: state_value(&state)?,
            })
        })
        .transpose()
    }

    /// Loads one revision shard through a collection- and source-scoped graph record.
    ///
    /// # Errors
    /// Missing/unauthorized record, mismatched graph pin, corrupt shard or store failure.
    pub fn representation_shard(
        &self,
        scopes: &ScopeSet,
        key: &RepresentationKey<'_>,
        revision_id: &str,
    ) -> Result<Option<RepresentationShard>, Error> {
        let found = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT r.shard_digest,g.graph_digest FROM representation_revisions AS r
             JOIN revision_unit_graphs AS g ON g.collection_id=r.collection_id
               AND g.chunk_set_id=r.chunk_set_id AND g.revision_id=r.revision_id
             WHERE r.collection_id=?1 AND r.chunk_set_id=?2 AND r.representation_set_id=?3
               AND r.revision_id=?4 AND {}",
                    ScopeSet::collection_condition("r.collection_id", 5)
                ),
                params![
                    key.collection_id,
                    key.chunk_set_id,
                    key.id,
                    revision_id,
                    scopes.parameter()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((digest, graph_digest)) = found else {
            return Ok(None);
        };
        let shard = load(
            self,
            &Digest::parse(&digest).map_err(|_| Error::Invalid("stored shard digest"))?,
        )?;
        require(
            shard.representation_set_id == key.id && shard.revision_id == revision_id,
            "shard record differs from artifact",
        )?;
        require(
            shard.graph_digest.as_str() == graph_digest,
            "shard graph pin differs from record",
        )?;
        Ok(Some(shard))
    }

    /// Reads a generation's optional SQL representation metadata; no shard is loaded.
    ///
    /// # Errors
    /// Invalid stored metadata or database failure.
    pub fn generation_representation(
        &self,
        scopes: &ScopeSet,
        generation: i64,
    ) -> Result<Option<RepresentationSet>, Error> {
        let found = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT r.collection_id,r.chunk_set_id,r.id,r.profile_digest,
                    r.embedding_profile,r.sparse_profile,r.layout,r.state
             FROM generation_representations AS b
             JOIN representation_sets AS r
               ON r.collection_id=b.collection_id AND r.chunk_set_id=b.chunk_set_id
                 AND r.id=b.representation_set_id
             WHERE b.generation_id=?1 AND {}",
                    ScopeSet::collection_condition("b.collection_id", 2)
                ),
                params![generation, scopes.parameter()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        found
            .map(
                |(collection, set, id, profile, dense, sparse, version, state)| {
                    Ok(RepresentationSet {
                        collection_id: collection,
                        chunk_set_id: set,
                        id,
                        profile_digest: Digest::parse(&profile)
                            .map_err(|_| Error::Invalid("stored profile digest"))?,
                        layout: RepresentationLayout {
                            version,
                            embedding_profile: dense,
                            sparse_profile: sparse,
                        },
                        state: state_value(&state)?,
                    })
                },
            )
            .transpose()
    }
}

/// Parses the stored representation lifecycle state.
fn state_value(state: &str) -> Result<RepresentationState, Error> {
    match state {
        "building" => Ok(RepresentationState::Building),
        "complete" => Ok(RepresentationState::Complete),
        "failed" => Ok(RepresentationState::Failed),
        _ => Err(Error::Invalid("stored representation state")),
    }
}
