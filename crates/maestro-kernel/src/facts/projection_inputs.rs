//! Scoped authoritative preflight for explicitly pinned projection resolutions.
use super::{
    error::Error,
    projection_binding::{EXACT_RESOLVER_VERSION, InputMismatchKind},
};
use crate::{artifact::Digest, scope::ScopeSet, store::Database};
use rusqlite::params;

impl Database {
    /// Validate a covering frozen resolution before any producer creates native storage.
    ///
    /// # Errors
    /// Refuses missing, foreign, unsupported or mismatching resolution inputs.
    pub fn validate_projection_inputs(
        &self,
        scopes: &ScopeSet,
        claim_set: &Digest,
        resolution: &Digest,
        resolver: &str,
    ) -> Result<(), Error> {
        let matches: bool = self.reader()?.query_row(
            &format!(
                "SELECT EXISTS (
                SELECT 1 FROM graph_resolutions r
                WHERE r.id = ?1 AND json_extract(r.body, '$.resolver_version') = ?2
                AND EXISTS (SELECT 1 FROM json_each(r.body, '$.sets') s WHERE s.value = ?3)
                AND NOT EXISTS (
                    SELECT 1 FROM json_each(r.body, '$.sets') s
                    LEFT JOIN claim_sets c ON c.id = s.value
                    WHERE c.id IS NULL OR NOT ({})
                ))",
                ScopeSet::collection_condition("c.collection_id", 4)
            ),
            params![
                resolution.as_str(),
                resolver,
                claim_set.as_str(),
                scopes.parameter()
            ],
            |row| row.get(0),
        )?;
        if resolver != EXACT_RESOLVER_VERSION || !matches {
            return Err(Error::ProjectionInputMismatch(
                InputMismatchKind::Resolution,
            ));
        }
        Ok(())
    }
}
