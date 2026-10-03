//! CLI health ports delegated to the opaque read-only knowledge probe.

use super::{check::Check, graph::check_with};
use crate::{cli::setup::graph::DIRECTORY, failure::Failure, settings::Session};
use maestro_canonicalization::SystemFileLock;
use maestro_kernel::{
    paths::{self, Environment},
    scope::Config,
};
use maestro_knowledge::graph::projection::{
    health::{ProbeError, PublishedGraph, Receipt},
    probe::Probe,
};

/// This adapter is compiled only with native support.
const ENGINE_BUILT: bool = true;

/// Lazily resolved application locations; constructing this makes zero calls.
struct Published<'a> {
    /// Application-resolved directory locations.
    environment: &'a Environment,
    /// The already-validated local read policy, never reread or persisted here.
    config: Option<&'a Config>,
}
impl<'a> Published<'a> {
    /// Delay every receipt/filesystem/engine operation until health selects it.
    fn new(environment: &'a Environment, config: Option<&'a Config>) -> Self {
        Self {
            environment,
            config,
        }
    }
}
impl PublishedGraph for Published<'_> {
    fn receipt(&self) -> Result<Receipt<'_>, ProbeError> {
        let data = paths::data_dir(self.environment).map_err(|_| ProbeError::AuthorityMissing)?;
        let config = self.config.ok_or(ProbeError::InventoryUnreadable)?;
        Ok(
            Probe::receipt(&data, &data.join(DIRECTORY), config, &SystemFileLock, None)?
                .into_receipt(),
        )
    }
}
/// Production graph check; the lazy adapter is never called while disabled.
pub(super) fn check(
    environment: &Environment,
    session: Result<Session, Failure>,
    config: Option<&Config>,
) -> Check {
    check_with(
        environment,
        session,
        ENGINE_BUILT,
        &Published::new(environment, config),
    )
}
