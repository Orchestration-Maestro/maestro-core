//! The kernel as every command opens it: its directories resolved from the
//! environment, its database opened, `config.toml` applied to the local
//! principal, and every read made through that principal's `ScopeSet`. In
//! the data directory only `kernel.sqlite3` and `artifacts/` are touched:
//! the files maestro v1 left there are never read, moved or removed.

use super::failure::Failure;
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{ModelCard, Role},
    paths::{self, Environment},
    scope::{Config, LOCAL, ScopeSet},
    store::Database,
};
use std::path::PathBuf;

/// The kernel, opened for the local principal.
#[derive(Debug)]
pub(super) struct Kernel {
    /// Its database, with the artifacts it records.
    pub(super) database: Database,
    /// The same artifact store, for strict model-card loading.
    pub(super) artifacts: Store,
    /// What the local principal reads: every read goes through it.
    pub(super) scopes: ScopeSet,
    /// Its configuration directory, which holds `bindings.toml`.
    pub(super) config_dir: PathBuf,
}

impl Kernel {
    /// The kernel of the directories the environment names, with the local
    /// principal's grants reconciled with `config.toml` first.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] when `config.toml` is not what it must be, and
    /// [`Failure::Failed`] when no directory can be resolved or the database
    /// cannot be opened, migrated or written.
    pub(super) fn open() -> Result<Self, Failure> {
        let environment = Environment::current();
        let data = paths::data_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
        let config_dir =
            paths::config_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
        let config = Config::load(&config_dir).map_err(|error| Failure::refused_by(&error))?;
        let database = Database::open_in(&data).map_err(|error| Failure::failed_by(&error))?;
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
            config_dir,
        })
    }

    /// The recorded model card `digest` when it is an embedder's.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] when the artifact is not recorded as a model card
    /// or its role is not an embedder's, and [`Failure::Failed`] when the
    /// kernel fails to read it.
    pub(super) fn embedder_card(&self, digest: &str) -> Result<ModelCard, Failure> {
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
