//! The scopes the kernel knows, which a grant may reach: its workspace, where
//! it keeps its records, and the scope of each collection and source it
//! records. `maestro doctor` names each grant that reaches none of them.

use super::{
    path::Scope,
    set::{ScopeSet, WORKSPACE},
};
use crate::store::{self, Database};
use rusqlite::{params, types::Type};

impl Database {
    /// The scopes the kernel knows that `scopes` covers, in path order: its
    /// workspace, `workspace/default`, and the scope of each collection and
    /// each source it records.
    ///
    /// # Errors
    ///
    /// [`store::Error::Sqlite`] when the database cannot be read, or records
    /// a collection or source whose scope it cannot read back.
    pub fn known_scopes(&self, scopes: &ScopeSet) -> Result<Vec<Scope>, store::Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT known.scope FROM (
               SELECT ?2 AS scope
               UNION ALL SELECT ?2 || '/collection/' || id FROM collections
               UNION ALL SELECT ?2 || '/collection/' || collection_id || '/source/' || id
                 FROM sources
             ) AS known
             WHERE {} ORDER BY known.scope",
            ScopeSet::condition("known.scope", 1)
        ))?;
        let known = statement
            .query_map(params![scopes.parameter(), WORKSPACE], |row| {
                let path: String = row.get(0)?;
                path.parse().map_err(|invalid| {
                    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(invalid))
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(known)
    }
}
