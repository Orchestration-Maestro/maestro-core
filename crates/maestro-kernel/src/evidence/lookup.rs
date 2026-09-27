//! Scoped chunk membership in retained, readable generations.

use super::error::Error;
use crate::{scope::ScopeSet, store::Database};
use rusqlite::{Row, params};

/// A chunk's membership in a published or explicitly selected retained generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkLocation {
    /// The collection the generation projects.
    pub collection_id: String,
    /// The admitted generation; later publication cannot move this identity.
    pub generation_id: i64,
    /// The chunk set pinned by the generation.
    pub chunk_set_id: String,
    /// The revision title when recorded as text.
    pub title: Option<String>,
}

impl Database {
    /// Finds a chunk only through source-visible membership in a published generation,
    /// or in the specified published or retired generation. Results are ordered by
    /// collection and generation and limited to two so callers can refuse ambiguity.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn chunk_locations(
        &self,
        scopes: &ScopeSet,
        chunk_id: &str,
        collection_id: Option<&str>,
        generation_id: Option<i64>,
    ) -> Result<Vec<ChunkLocation>, Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT generations.collection_id, generations.id, generations.chunk_set_id,
                    CASE json_type(revisions.metadata_json, '$.title')
                      WHEN 'text' THEN json_extract(revisions.metadata_json, '$.title') END
             FROM generations
             JOIN chunks ON chunks.chunk_set_id = generations.chunk_set_id
             JOIN revisions ON revisions.id = chunks.revision_id
             JOIN documents ON documents.id = revisions.document_id
             WHERE chunks.id = ?1
               AND (?2 IS NULL OR generations.collection_id = ?2)
               AND (?3 IS NULL OR ?2 IS NOT NULL)
               AND ((?3 IS NULL AND generations.state = 'published') OR
                    (?3 IS NOT NULL AND generations.id = ?3
                     AND generations.state IN ('published', 'retired')))
               AND {}
             ORDER BY generations.collection_id, generations.id
             LIMIT 2",
            ScopeSet::source_condition("documents.collection_id", "documents.source_id", 4)
        ))?;
        let locations = statement
            .query_map(
                params![chunk_id, collection_id, generation_id, scopes.parameter()],
                location_row,
            )?
            .collect::<Result<_, _>>()?;
        Ok(locations)
    }
}

/// Reads one row of [`Database::chunk_locations`].
fn location_row(row: &Row<'_>) -> rusqlite::Result<ChunkLocation> {
    Ok(ChunkLocation {
        collection_id: row.get(0)?,
        generation_id: row.get(1)?,
        chunk_set_id: row.get(2)?,
        title: row.get(3)?,
    })
}
