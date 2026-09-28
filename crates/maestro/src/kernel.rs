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
    scope::{Config, LOCAL, ScopeSet},
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

/// One-shot callback used by tests to revoke access during a read.
#[cfg(test)]
pub(crate) type RefreshHook = fn(&Kernel);

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
        let config = Config::load(config_dir).map_err(|error| Failure::refused_by(&error))?;
        let database =
            Arc::new(Database::open_in(data).map_err(|error| Failure::failed_by(&error))?);
        let artifacts = Store::new(data.join("artifacts"));
        database
            .apply_config(&config)
            .map_err(|error| Failure::failed_by(&error))?;
        let scopes = database
            .visible(LOCAL)
            .map_err(|error| Failure::failed_by(&error))?;
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
        #[cfg(test)]
        if let Some((remaining, hook)) = self.test_refresh_hook.take() {
            if remaining == 0 {
                hook(self);
            } else {
                self.test_refresh_hook = Some((remaining - 1, hook));
            }
        }
        let config = Config::load(&self.config_dir).map_err(|error| Failure::refused_by(&error))?;
        self.database
            .apply_config(&config)
            .map_err(|error| Failure::failed_by(&error))?;
        self.scopes = self
            .database
            .visible(LOCAL)
            .map_err(|error| Failure::failed_by(&error))?;
        Ok(())
    }

    /// Installs a one-shot permission-refresh hook for an isolated unit test.
    #[cfg(test)]
    pub(crate) fn with_test_refresh_hook(
        mut self,
        after_refreshes: usize,
        hook: fn(&Self),
    ) -> Self {
        self.test_refresh_hook = Some((after_refreshes, hook));
        self
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
