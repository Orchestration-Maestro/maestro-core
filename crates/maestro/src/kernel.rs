//! The kernel as every command opens it: directories resolved from the
//! environment, the database opened, `config.toml` applied to the local
//! principal, and every read made through its `ScopeSet`. In the data
//! directory only `kernel.sqlite3` and `artifacts/` are touched: maestro v1
//! files there are never read, moved or removed. Plan D12's failure boundary
//! is shared by the CLI and MCP; permission refreshes stay on one connection.

use crate::failure::Failure;
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{ModelCard, Role},
    paths::{self, Environment},
    scope::{ConfigRefreshError, ScopeSet},
    store::Database,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// The embedder pinned by a generation, loaded from the artifact store.
/// Missing, malformed, corrupt, or non-embedder cards make dense unavailable.
pub(crate) fn pinned_embedder(artifacts: &Store, profile: Option<&str>) -> Option<ModelCard> {
    let profile = profile?.strip_prefix("dense/1:sha256:")?;
    let digest = Digest::parse(profile).ok()?;
    let card = ModelCard::load(artifacts, &digest).ok()?;
    (card.fields().role == Role::Embedder).then_some(card)
}

/// One-shot callbacks used to order revocation and a competing writer.
#[cfg(test)]
#[derive(Debug)]
pub(crate) enum RefreshHook {
    /// Changes the config before the refresh loads it.
    BeforeLoad(fn(&Kernel)),
    /// Schedules a delayed writer at the refresh's snapshot boundary.
    PendingWriter(fn(&Kernel)),
}

/// The kernel, opened for the local principal.
#[derive(Debug)]
pub(crate) struct Kernel {
    /// Its database, with the artifacts it records.
    pub(crate) database: Arc<Database>,
    /// The same artifact store, for strict model-card loading.
    pub(crate) artifacts: Store,
    /// What the local principal reads: every read goes through it.
    pub(crate) scopes: ScopeSet,
    /// Its configuration directory, which holds `bindings.toml`.
    pub(crate) config_dir: PathBuf,
    /// Test-only callback invoked before a permission refresh.
    #[cfg(test)]
    pub(crate) test_refresh_hook: Option<(usize, RefreshHook)>,
}

impl Kernel {
    /// The kernel of the directories the environment names, with local grants
    /// reconciled with `config.toml` first.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] when `config.toml` is invalid, and
    /// [`Failure::Failed`] when directories cannot be resolved or the database
    /// cannot be opened, migrated or written.
    pub(crate) fn open() -> Result<Self, Failure> {
        let environment = Environment::current();
        let data = paths::data_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
        let config_dir =
            paths::config_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
        Self::open_at(&data, &config_dir)
    }

    /// Opens a kernel at explicit data and configuration directories.
    pub(crate) fn open_at(data: &Path, config_dir: &Path) -> Result<Self, Failure> {
        let database =
            Arc::new(Database::open_in(data).map_err(|error| Failure::failed_by(&error))?);
        let artifacts = Store::new(data.join("artifacts"));
        let scopes = database
            .refresh_config(config_dir)
            .map_err(config_refresh_failure)?;
        Ok(Self {
            database,
            artifacts,
            scopes,
            config_dir: config_dir.to_path_buf(),
            #[cfg(test)]
            test_refresh_hook: None,
        })
    }

    /// Reloads the local grants on this database connection and refreshes its scope snapshot.
    pub(crate) fn refresh_scopes(&mut self) -> Result<(), Failure> {
        // A competing writer must finish before the atomic refresh's lock;
        // there is no longer a gap between reconciliation and its snapshot.
        #[cfg(test)]
        if let Some(writer) = self.test_pending_writer() {
            writer(self);
        }
        self.scopes = self
            .database
            .refresh_config(&self.config_dir)
            .map_err(config_refresh_failure)?;
        Ok(())
    }

    /// Installs a one-shot permission-refresh hook for an isolated unit test.
    #[cfg(test)]
    pub(crate) fn with_test_refresh_hook(
        mut self,
        after_refreshes: usize,
        hook: fn(&Self),
    ) -> Self {
        self.test_refresh_hook = Some((after_refreshes, RefreshHook::BeforeLoad(hook)));
        self
    }

    /// Orders a competing writer at the final permission snapshot in a test.
    #[cfg(test)]
    pub(crate) fn with_test_pending_writer(mut self, writer: fn(&Self)) -> Self {
        self.test_refresh_hook = Some((0, RefreshHook::PendingWriter(writer)));
        self
    }

    /// Runs due config edits and returns a due competing writer.
    #[cfg(test)]
    fn test_pending_writer(&mut self) -> Option<fn(&Self)> {
        let (remaining, hook) = self.test_refresh_hook.take()?;
        if remaining > 0 {
            self.test_refresh_hook = Some((remaining - 1, hook));
            return None;
        }
        match hook {
            RefreshHook::BeforeLoad(edit) => {
                edit(self);
                None
            }
            RefreshHook::PendingWriter(writer) => Some(writer),
        }
    }

    /// The recorded model card `digest` when it is an embedder's.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] when the artifact is not a model card or its role
    /// is not an embedder's, and [`Failure::Failed`] when the kernel cannot
    /// read it.
    pub(crate) fn embedder_card(&self, digest: &str) -> Result<ModelCard, Failure> {
        let digest = Digest::parse(digest).map_err(|error| Failure::refused_by(&error))?;
        if self
            .database
            .artifact(&digest)
            .map_err(|error| Failure::failed_by(&error))?
            .is_none()
        {
            return Err(Failure::refused(format!(
                "no model card sha256:{} is recorded",
                digest.as_str()
            )));
        }
        let card = ModelCard::load(&self.artifacts, &digest)
            .map_err(|error| Failure::refused_by(&error))?;
        if card.fields().role != Role::Embedder {
            return Err(Failure::refused(format!(
                "model card sha256:{} is a {}'s, not an embedder's",
                digest.as_str(),
                card.fields().role
            )));
        }
        Ok(card)
    }
}

/// Preserves the CLI boundary between invalid configuration and database failure.
fn config_refresh_failure(error: ConfigRefreshError) -> Failure {
    match error {
        ConfigRefreshError::Config(error) => Failure::refused_by(&error),
        ConfigRefreshError::Store(error) => Failure::failed_by(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::Kernel;
    use crate::failure::Failure;
    use maestro_kernel::{scope::LOCAL, store::Database};
    use maestro_test_scratch::scratch_directory;
    use rusqlite::Connection;
    use std::fs;

    #[test]
    fn open_at_config_refresh_takes_the_writer_lock_before_loading() {
        assert_lock_precedes_config_load(false);
    }

    #[test]
    fn refresh_scopes_config_refresh_takes_the_writer_lock_before_loading() {
        assert_lock_precedes_config_load(true);
    }

    /// A malformed file must not be read until the writer lock is available.
    /// Each call waits out the kernel's five-second busy timeout: the only
    /// lock signal a caller outside the kernel crate can observe.
    fn assert_lock_precedes_config_load(refresh: bool) {
        let directory = scratch_directory().unwrap();
        let mut kernel = Kernel::open_at(&directory, &directory).unwrap();
        let previous = kernel.scopes.clone();
        let outside = Connection::open(directory.join("kernel.sqlite3")).unwrap();
        outside.execute_batch("BEGIN IMMEDIATE").unwrap();
        fs::write(directory.join("config.toml"), "[unknown]\n").unwrap();
        let result = if refresh {
            kernel.refresh_scopes()
        } else {
            Kernel::open_at(&directory, &directory).map(|_| ())
        };
        assert!(
            matches!(
                &result,
                Err(Failure::Failed(message)) if message.contains("database is locked")
            ),
            "atomic config refresh must encounter the writer lock before invalid config: {result:?}"
        );
        assert_eq!(kernel.scopes, previous);
        outside.execute_batch("ROLLBACK").unwrap();
        assert_eq!(kernel.database.visible(LOCAL).unwrap(), previous);
        // Once the lock is available, the invalid config is a refusal, proving
        // that opening the already-migrated database itself was not the failure.
        let reopened = Database::open_in(&directory).unwrap();
        assert!(matches!(kernel.refresh_scopes(), Err(Failure::Refused(_))));
        assert!(matches!(
            Kernel::open_at(&directory, &directory),
            Err(Failure::Refused(_))
        ));
        drop(reopened);
        drop(outside);
        drop(kernel);
        fs::remove_dir_all(directory).unwrap();
    }
}
