//! Canonical shard validation and relational set completion.
use super::{
    error::{Error, require},
    types::{RepresentationKey, RepresentationShard},
};
use crate::{artifact::Digest, scope::ScopeSet, store::Database, unit_graph::GraphKey};
use rusqlite::params;
use serde::{Serialize, de::DeserializeOwned};

/// Decodes canonical representation JSON without using the graph decoder seam.
fn decode<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, Error> {
    let value: T = serde_json::from_slice(bytes)?;
    require(serde_json::to_vec(&value)? == bytes, "noncanonical JSON")?;
    Ok(value)
}

impl RepresentationShard {
    /// Serializes a revision shard in its declared field order.
    ///
    /// # Errors
    /// JSON serialization failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Parses a canonical shard and checks its ordered member ordinals.
    ///
    /// # Errors
    /// Unknown/duplicate fields, unsupported schema or invalid member order.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let shard: Self = decode(bytes)?;
        require(
            shard.schema_version == "maestro-representation-shard/1",
            "unsupported shard schema",
        )?;
        require(
            !shard.representation_set_id.trim().is_empty() && !shard.revision_id.trim().is_empty(),
            "empty shard identity",
        )?;
        for (ordinal, member) in shard.members.iter().enumerate() {
            require(
                member.ordinal == ordinal as u64,
                "shard member ordinal mismatch",
            )?;
        }
        Ok(shard)
    }
}

/// Confirms a shard is exactly the graph's view/membership sequence.
pub(super) fn validate_shard(
    db: &Database,
    scopes: &ScopeSet,
    set: &RepresentationKey<'_>,
    shard: &RepresentationShard,
) -> Result<(), Error> {
    require(
        shard.representation_set_id == set.id,
        "shard belongs to another set",
    )?;
    let key = GraphKey {
        collection_id: set.collection_id,
        chunk_set_id: set.chunk_set_id,
        revision_id: &shard.revision_id,
    };
    let graph = db.unit_graph(scopes, &key)?.ok_or(Error::NotFound)?;
    let mut actual = shard.members.iter();
    for view in &graph.retrieval_views {
        for membership in &view.memberships {
            let member = actual
                .next()
                .ok_or(Error::Invalid("partial revision shard"))?;
            require(
                member.retrieval_view_id == view.retrieval_view_id
                    && member.chunk_id == view.chunk_id
                    && member.unit_id == membership.unit_id
                    && member.input_digest == view.prepared_input_digest
                    && member.mapping_digest == graph.descriptor.mapping_digest,
                "shard member differs from graph membership",
            )?;
        }
    }
    require(actual.next().is_none(), "extra revision shard members")
}

/// Requires a frozen chunk set and exactly one shard per recorded graph revision.
pub(super) fn complete(
    db: &Database,
    scopes: &ScopeSet,
    key: &RepresentationKey<'_>,
) -> Result<(), Error> {
    let reader = db.reader()?;
    let frozen = reader
        .prepare(&format!(
            "SELECT 1 FROM chunk_sets WHERE collection_id=?1 AND id=?2 AND state='complete' AND {}",
            ScopeSet::collection_condition("chunk_sets.collection_id", 3)
        ))?
        .exists(params![
            key.collection_id,
            key.chunk_set_id,
            scopes.parameter()
        ])?;
    require(frozen, "representation requires a complete chunk set")?;
    let missing_graph = reader
        .prepare(
            "SELECT 1 FROM chunks AS c WHERE c.chunk_set_id=?1 AND NOT EXISTS
         (SELECT 1 FROM revision_unit_graphs AS g WHERE g.collection_id=?2
          AND g.chunk_set_id=c.chunk_set_id AND g.revision_id=c.revision_id) LIMIT 1",
        )?
        .exists(params![key.chunk_set_id, key.collection_id])?;
    let missing = reader
        .prepare(
            "SELECT 1 FROM revision_unit_graphs AS g WHERE g.collection_id=?1 AND g.chunk_set_id=?2
         AND NOT EXISTS (SELECT 1 FROM representation_revisions AS r
           WHERE r.collection_id=g.collection_id AND r.chunk_set_id=g.chunk_set_id
             AND r.revision_id=g.revision_id AND r.representation_set_id=?3)",
        )?
        .exists(params![key.collection_id, key.chunk_set_id, key.id])?;
    let extra = reader
        .prepare(
            "SELECT 1 FROM representation_revisions AS r
         WHERE r.collection_id=?1 AND r.chunk_set_id=?2
           AND r.representation_set_id=?3
           AND NOT EXISTS (SELECT 1 FROM revision_unit_graphs AS g
             WHERE g.collection_id=r.collection_id AND g.chunk_set_id=r.chunk_set_id
               AND g.revision_id=r.revision_id)",
        )?
        .exists(params![key.collection_id, key.chunk_set_id, key.id])?;
    require(
        !missing_graph && !missing && !extra,
        "every chunk revision must have one graph and shard",
    )
}

/// Loads a stored CAS shard; CAS get already verifies the artifact digest.
pub(super) fn load(db: &Database, digest: &Digest) -> Result<RepresentationShard, Error> {
    RepresentationShard::from_bytes(&db.get(digest)?)
}
