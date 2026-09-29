//! Atomic graph membership and artifact pins, after existing chunk recording.

use super::{
    error::{Error, require},
    read::load,
    types::{DeliveryGraph, RankPolicy},
};
use crate::{
    artifact::Digest,
    scope::ScopeSet,
    store::{Database, artifacts},
};
use rusqlite::{OptionalExtension as _, Transaction, params};
use std::collections::BTreeMap;

impl Database {
    /// Validates a stored graph against its scoped revision and existing chunks,
    /// then records membership and pins graph/mapping artifacts in one write.
    /// An exact repeat is a no-op; changed content is never replaced.
    ///
    /// # Errors
    /// Unknown/unauthorized revision, unknown or mismatched chunks, invalid
    /// artifacts, profile conflict or a chunk set no longer building.
    pub fn record_revision_graph(
        &self,
        scopes: &ScopeSet,
        chunk_set_id: &str,
        digest: &Digest,
    ) -> Result<(), Error> {
        let graph = load(self, digest)?;
        self.write(|tx| {
            authorize(tx, scopes, chunk_set_id, &graph)?;
            let descriptor = &graph.descriptor;
            let existing: Option<String> = tx
                .query_row(
                    "SELECT graph_digest FROM revision_unit_graphs
                 WHERE collection_id=?1 AND chunk_set_id=?2 AND revision_id=?3",
                    params![
                        descriptor.collection_id,
                        chunk_set_id,
                        descriptor.revision_id
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(existing) = existing {
                return (existing == digest.as_str())
                    .then_some(())
                    .ok_or(Error::Conflict);
            }
            chunks(tx, chunk_set_id, &graph)?;
            profile(tx, chunk_set_id, &graph)?;
            tx.execute(
                "INSERT INTO revision_unit_graphs VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    descriptor.collection_id,
                    chunk_set_id,
                    descriptor.revision_id,
                    descriptor.document_id,
                    digest.as_str(),
                    descriptor.mapping_digest.as_str(),
                    descriptor.schema_version,
                    descriptor.original_markdown_digest.as_str(),
                ],
            )?;
            artifacts::pin(tx, digest)?;
            artifacts::pin(tx, &descriptor.mapping_digest)?;
            Ok(())
        })
    }
}

/// Validates scope and identity inside the membership transaction.
fn authorize(
    tx: &Transaction<'_>,
    scopes: &ScopeSet,
    set: &str,
    graph: &DeliveryGraph,
) -> Result<(), Error> {
    let descriptor = &graph.descriptor;
    let authorized = tx
        .prepare(&format!(
            "SELECT 1 FROM revisions AS r JOIN documents AS d ON d.id=r.document_id
         JOIN chunk_sets AS c ON c.collection_id=d.collection_id
         WHERE c.id=?1 AND d.collection_id=?2 AND r.id=?3 AND d.id=?4
           AND r.original_digest=?5 AND {}",
            ScopeSet::source_condition("d.collection_id", "d.source_id", 6)
        ))?
        .exists(params![
            set,
            descriptor.collection_id,
            descriptor.revision_id,
            descriptor.document_id,
            descriptor.original_markdown_digest.as_str(),
            scopes.parameter()
        ])?;
    if authorized {
        Ok(())
    } else {
        Err(Error::NotFound)
    }
}

/// One indexed revision read verifies every referenced chunk and its count/input.
fn chunks(tx: &Transaction<'_>, set: &str, graph: &DeliveryGraph) -> Result<(), Error> {
    let mut statement = tx.prepare(
        "SELECT id,digest,token_count FROM chunks WHERE chunk_set_id=?1 AND revision_id=?2",
    )?;
    let recorded: BTreeMap<String, (String, i64)> = statement
        .query_map(params![set, graph.descriptor.revision_id], |row| {
            Ok((row.get(0)?, (row.get(1)?, row.get(2)?)))
        })?
        .collect::<Result<_, _>>()?;
    require(
        recorded.len() == graph.retrieval_views.len(),
        "graph must represent every revision chunk",
    )?;
    for view in &graph.retrieval_views {
        require(
            recorded.get(&view.chunk_id)
                == Some(&(
                    view.prepared_input_digest.as_str().to_owned(),
                    i64::try_from(view.token_count)
                        .map_err(|_| Error::Invalid("token count overflow"))?,
                )),
            "unknown chunk or prepared digest/count mismatch",
        )?;
    }
    Ok(())
}

/// Inserts or checks the one immutable extended profile of a chunk set.
fn profile(tx: &Transaction<'_>, set: &str, graph: &DeliveryGraph) -> Result<(), Error> {
    let descriptor = &graph.descriptor;
    let policy = match graph
        .retrieval_views
        .first()
        .ok_or(Error::Invalid("graph has no retrieval view"))?
        .rank_policy
    {
        RankPolicy::CompleteIdeas => "complete_ideas",
        RankPolicy::V2Unit => "v2_unit",
    };
    let expected = [
        descriptor.profile_name.as_str(),
        descriptor.profile_digest.as_str(),
        descriptor.preparation_name.as_str(),
        descriptor.preparation_digest.as_str(),
        descriptor.counter_contract.as_str(),
        policy,
    ];
    let existing: Option<Vec<String>> = tx
        .query_row(
            "SELECT profile_name,profile_digest,preparation_name,preparation_digest,
                counter_contract,rank_policy
         FROM chunk_set_profiles WHERE collection_id=?1 AND chunk_set_id=?2",
            params![descriptor.collection_id, set],
            |row| (0..6).map(|i| row.get(i)).collect(),
        )
        .optional()?;
    if let Some(existing) = existing {
        return require(existing == expected, "chunk set graph profile mismatch");
    }
    tx.execute(
        "INSERT INTO chunk_set_profiles VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            descriptor.collection_id,
            set,
            expected[0],
            expected[1],
            expected[2],
            expected[3],
            expected[4],
            expected[5]
        ],
    )?;
    Ok(())
}
