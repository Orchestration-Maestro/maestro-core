//! Rooted immutable native reads, bound to one scope and the exact physical receipt file.

use super::{open::open, rows};
use crate::graph::projection::{
    cancellation::ProjectionCancellation,
    content,
    port::{EdgeFamily, EntityFact, ProjectionEdge, ProjectionScope},
    writer::{BuildVerification, ProjectionBackendReader, receipt_from_verification},
};
use lbug::{Connection, Database, RootDirectory, SystemConfig};
use maestro_kernel::{artifact::Digest, facts::ProjectionReceipt};

/// Read-only native handle to one immutable physical projection file.
pub(in crate::graph::projection) struct Reader {
    /// Held until the reader drops; no writable handle is retained here.
    database: Database,
    /// Exact scope checked in the durable schema stamp.
    scope: ProjectionScope,
}
impl Reader {
    /// Open a rooted child read-only; callers verify before exposing any rows.
    pub(super) fn open(
        root: &RootDirectory,
        name: &str,
        config: SystemConfig,
        scope: &ProjectionScope,
    ) -> Result<Self, String> {
        let reader = Self {
            database: open(root, name, config.read_only(true))
                .map_err(|error| error.to_string())?,
            scope: scope.clone(),
        };
        Ok(reader)
    }

    /// Bind the physical basename, scope, counts and content to the kernel receipt.
    pub(in crate::graph::projection) fn published(
        root: &RootDirectory,
        config: SystemConfig,
        scope: &ProjectionScope,
        receipt: &ProjectionReceipt,
    ) -> Result<Self, String> {
        if content::basename(scope, &receipt.claim_set_id)? != receipt.file_name {
            return Err(
                "native projection receipt does not name this scope's physical file".into(),
            );
        }
        let reader = Self::open(root, &receipt.file_name, config, scope)?;
        let mapped = receipt_from_verification(
            scope,
            receipt.claim_set_id.clone(),
            receipt.file_name.clone(),
            &reader.verification()?,
        )
        .map_err(|error| error.to_string())?;
        if mapped != *receipt {
            return Err("native projection physical content does not match its receipt".into());
        }
        Ok(reader)
    }

    /// Run the same strict reads with a scoped native interrupt relay.
    pub(super) fn cancellable_rows(
        &self,
        token: &ProjectionCancellation,
    ) -> Result<rows::Rows, String> {
        let connection = Connection::new(&self.database).map_err(|error| error.to_string())?;
        super::cancellation::run(&connection, token, || rows::read(&connection, &self.scope))
    }

    /// Validate canonical rows in the same immutable handle on every read.
    fn rows(&self) -> Result<rows::Rows, String> {
        // ponytail: validate all rows per read; use scoped index reads
        // if query measurements need it.
        rows::read(
            &Connection::new(&self.database).map_err(|error| error.to_string())?,
            &self.scope,
        )
    }
}
impl ProjectionBackendReader for Reader {
    fn verification(&self) -> Result<BuildVerification, String> {
        self.rows()?.verification()
    }

    fn edges_adjacent(
        &self,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, String> {
        Ok(self
            .rows()?
            .edges
            .into_iter()
            .filter(|edge| {
                edge.family == family && (edge.source == *entity || edge.target == *entity)
            })
            .collect())
    }

    fn facts_for(&self, subject: &Digest) -> Result<Vec<EntityFact>, String> {
        Ok(self
            .rows()?
            .facts
            .into_iter()
            .filter(|fact| fact.subject == *subject)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(windows))]
    use crate::graph::projection::engine::schema;
    use crate::graph::projection::engine::tests::{Fixture, config, scope};
    #[cfg(windows)]
    use crate::graph::projection::engine::{
        open::tests::private_windows_fixture, schema::tests::install_reader_fixture,
    };
    use std::fs;

    #[test]
    fn native_reader_forces_read_only_even_when_caller_config_is_writable() {
        let fixture = Fixture::new();
        {
            let database = fixture.writer();
            let connection = Connection::new(&database).unwrap();
            #[cfg(not(windows))]
            schema::create(&connection, &scope()).unwrap();
            #[cfg(windows)]
            install_reader_fixture(&connection, &scope());
            connection.query("CHECKPOINT").unwrap();
        }
        #[cfg(windows)]
        private_windows_fixture(&fixture.path);
        let before = fs::read(fixture.path.join("rows.lbdb")).unwrap();
        let root = RootDirectory::open(&fixture.path).unwrap();
        let reader = Reader::open(&root, "rows.lbdb", config(), &scope()).unwrap();
        assert!(
            Connection::new(&reader.database)
                .unwrap()
                .query("CREATE (:Entity {id: 'probe', facts: []})")
                .is_err()
        );
        assert_eq!(reader.verification().unwrap().fact_count, 0);
        drop(reader);
        assert_eq!(fs::read(fixture.path.join("rows.lbdb")).unwrap(), before);
    }
}
