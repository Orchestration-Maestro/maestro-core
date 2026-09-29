//! One authorized candidate revision maps to one immutable graph artifact.

use super::{
    error::{Error, require},
    hierarchy,
    mapping::MappingLedger,
    types::DeliveryGraph,
};
use crate::{artifact::Digest, scope::ScopeSet, store::Database};
use rusqlite::{OptionalExtension as _, params};
use std::str;

impl DeliveryGraph {
    /// Derives required context from typed ancestor relations; headings are excluded.
    ///
    /// # Errors
    /// The unit or one of its ancestor groups is absent.
    pub fn required_context(&self, unit_id: &str) -> Result<Vec<String>, Error> {
        Ok(hierarchy::required_context(self, unit_id)?
            .into_iter()
            .map(str::to_owned)
            .collect())
    }
}

/// Fully scoped candidate identity; IDs from another revision cannot be followed.
#[derive(Clone, Copy, Debug)]
pub struct GraphKey<'a> {
    /// Owning collection.
    pub collection_id: &'a str,
    /// Immutable chunk set.
    pub chunk_set_id: &'a str,
    /// Exact source revision.
    pub revision_id: &'a str,
}

impl Database {
    /// Loads an authorized revision graph; unauthorized and absent are identical.
    /// The adapter caches this value for one request, never across scope changes.
    ///
    /// # Errors
    /// Store corruption, invalid wire bytes or a violated graph invariant.
    pub fn unit_graph(
        &self,
        scopes: &ScopeSet,
        key: &GraphKey<'_>,
    ) -> Result<Option<DeliveryGraph>, Error> {
        let found = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT graph_digest,mapping_digest,original_markdown_digest,g.document_id
             FROM revision_unit_graphs AS g JOIN documents AS d ON d.id=g.document_id
             WHERE g.collection_id=?1 AND g.chunk_set_id=?2 AND g.revision_id=?3 AND {}",
                    ScopeSet::source_condition("d.collection_id", "d.source_id", 4)
                ),
                params![
                    key.collection_id,
                    key.chunk_set_id,
                    key.revision_id,
                    scopes.parameter()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((digest, mapping, original, document)) = found else {
            return Ok(None);
        };
        let digest = Digest::parse(&digest).map_err(|_| Error::Invalid("stored graph digest"))?;
        let graph = load(self, &digest)?;
        let descriptor = &graph.descriptor;
        require(
            descriptor.collection_id == key.collection_id
                && descriptor.revision_id == key.revision_id
                && descriptor.document_id == document
                && descriptor.mapping_digest.as_str() == mapping
                && descriptor.original_markdown_digest.as_str() == original,
            "graph record differs from artifact",
        )?;
        Ok(Some(graph))
    }
}

/// Reads and validates the digest-bound graph and its source/mapping artifacts.
pub(crate) fn load(db: &Database, digest: &Digest) -> Result<DeliveryGraph, Error> {
    let graph = DeliveryGraph::from_bytes(&db.get(digest)?)?;
    let mapping = MappingLedger::from_bytes(&db.get(&graph.descriptor.mapping_digest)?)?;
    let bytes = db.get(&graph.descriptor.original_markdown_digest)?;
    let source = str::from_utf8(&bytes).map_err(|_| Error::Invalid("source is not UTF-8"))?;
    graph.validate_recorded(&mapping, source)?;
    Ok(graph)
}
