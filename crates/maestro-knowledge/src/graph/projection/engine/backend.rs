//! Private native projection adapter with lifecycle-owned publication and staging reservation.

use super::{open::open, reader::Reader, schema, transaction::Transactions};
use crate::graph::projection::{
    content,
    port::{EntityFact, ProjectionEdge, ProjectionScope},
    writer::{BuildVerification, ProjectionBackend, ProjectionBackendReader},
};
use lbug::{Connection, Database, RootDirectory, SystemConfig};
use maestro_kernel::facts::ProjectionReceipt;

/// Lifecycle-owned lease-bound, no-overwrite installation of a closed staging file.
/// The public lifecycle provides this implementation and reserves the staging name.
pub(in crate::graph::projection) trait Publication {
    /// Install exactly this reserved child under the canonical receipt basename.
    fn install(&mut self, staging: &str, published: &str) -> Result<(), String>;
}

/// One reserved unpublished native session, never a public factory.
pub(in crate::graph::projection) struct Backend<P: Publication> {
    /// Held root capability used by every native open.
    root: RootDirectory,
    /// Final immutable files live in a separate held root.
    final_root: RootDirectory,
    /// Explicit caller-owned settings, also reused for read-only opens.
    config: SystemConfig,
    /// Lifecycle-reserved staging child.
    staging: String,
    /// Lifecycle publication implementation.
    publication: P,
    /// Exact scope admitted once by schema creation.
    scope: Option<ProjectionScope>,
    /// At most one writable handle, dropped before every read-only verification.
    database: Option<Database>,
    /// Poison state survives native close/reopen.
    transactions: Transactions,
    /// Latest verification, invalidated before any attempted batch.
    verified: bool,
}
impl<P: Publication> Backend<P> {
    /// The caller reserves `staging` before construction; no filesystem publication lives here.
    pub(in crate::graph::projection) fn new(
        root: RootDirectory,
        final_root: RootDirectory,
        staging: String,
        config: SystemConfig,
        publication: P,
    ) -> Self {
        Self {
            root,
            final_root,
            staging,
            config,
            publication,
            scope: None,
            database: None,
            transactions: Transactions::default(),
            verified: false,
        }
    }

    /// Bind the prepared receipt to the lifecycle publication callback before closing.
    pub(in crate::graph::projection) fn publication_mut(&mut self) -> &mut P {
        &mut self.publication
    }

    /// Exercise actual native rollback poisoning through the test-only transaction helper.
    #[cfg(all(test, not(windows)))]
    pub(super) fn poison_for_test(&mut self) -> Result<(), String> {
        let connection = Connection::new(self.database.as_ref().ok_or("closed native session")?)
            .map_err(|error| error.to_string())?;
        super::transaction::tests::poison(&mut self.transactions, &connection);
        Ok(())
    }

    /// Refuse a different generation, a closed writer or a poisoned native session.
    fn usable(&self, scope: &ProjectionScope) -> Result<(), String> {
        self.transactions.ensure_usable()?;
        if self.scope.as_ref() != Some(scope) || self.database.is_none() {
            return Err("native projection has no writable session for this scope".into());
        }
        Ok(())
    }
}
impl<P: Publication> ProjectionBackend for Backend<P> {
    type Reader = Reader;

    #[cfg(test)]
    fn inject_batch_failure(&mut self) -> Result<(), String> {
        self.transactions.inject_batch_failure()
    }

    fn create_unpublished(&mut self, scope: &ProjectionScope) -> Result<(), String> {
        schema::writable(cfg!(windows))?;
        if self.scope.is_some() {
            return Err("native projection session has already been created".into());
        }
        let database = open(&self.root, &self.staging, self.config.clone())
            .map_err(|error| error.to_string())?;
        self.scope = Some(scope.clone());
        schema::create(
            &Connection::new(&database).map_err(|error| error.to_string())?,
            scope,
        )?;
        self.database = Some(database);
        Ok(())
    }

    fn write_batch(
        &mut self,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), String> {
        self.usable(scope)?;
        self.verified = false;
        let connection = Connection::new(self.database.as_ref().ok_or("closed native session")?)
            .map_err(|error| error.to_string())?;
        self.transactions
            .write_batch(&connection, scope, edges, facts)
    }

    fn verify_unpublished(&mut self, scope: &ProjectionScope) -> Result<BuildVerification, String> {
        self.usable(scope)?;
        self.verified = false;
        {
            let connection =
                Connection::new(self.database.as_ref().ok_or("closed native session")?)
                    .map_err(|error| error.to_string())?;
            connection
                .query("CHECKPOINT")
                .map_err(|error| error.to_string())?;
        }
        drop(self.database.take());
        let verified = Reader::open(&self.root, &self.staging, self.config.clone(), scope)
            .and_then(|reader| reader.verification());
        self.database = Some(
            open(&self.root, &self.staging, self.config.clone())
                .map_err(|error| error.to_string())?,
        );
        let verified = verified?;
        self.verified = true;
        Ok(verified)
    }

    fn publish_unpublished(
        &mut self,
        scope: &ProjectionScope,
        file_name: &str,
    ) -> Result<(), String> {
        self.usable(scope)?;
        if !self.verified || !content::is_canonical_basename(file_name) {
            return Err(
                "native projection publication requires verification and a receipt basename".into(),
            );
        }
        drop(self.database.take());
        self.publication.install(&self.staging, file_name)?;
        Ok(())
    }

    fn open_published(
        &self,
        scope: &ProjectionScope,
        receipt: &ProjectionReceipt,
    ) -> Result<Self::Reader, String> {
        Reader::published(&self.final_root, self.config.clone(), scope, receipt)
    }
}
