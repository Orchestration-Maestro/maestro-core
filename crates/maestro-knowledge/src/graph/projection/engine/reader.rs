//! Rooted immutable native reads, bound to one scope and the exact physical receipt file.

use super::{open::open, rows};
use crate::graph::projection::{
    EdgeFamily, EntityFact, ProjectionEdge, ProjectionScope, content,
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
    pub(super) fn published(
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
