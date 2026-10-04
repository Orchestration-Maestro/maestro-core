//! Engine-absent adapter: the selected engine is refused before inventory.

use super::{check::Check, graph::check_with};
use crate::{failure::Failure, settings::Session};
use maestro_kernel::{paths::Environment, scope::Config};
use maestro_knowledge::graph::projection::health::{ProbeError, PublishedGraph, Receipt};

/// This adapter is compiled only in engine-absent builds.
const ENGINE_BUILT: bool = false;

/// An absent engine publishes nothing, even when injected into a test.
struct Published;
impl Published {
    /// Construct without reading directories, receipts or guards.
    fn new(_: &Environment) -> Self {
        Self
    }
}
impl PublishedGraph for Published {
    fn receipt(&self) -> Result<Receipt<'_>, ProbeError> {
        Ok(Receipt::NonePublished)
    }
}

/// Production graph check; the lazy adapter is never called while disabled.
pub(super) fn check(
    environment: &Environment,
    session: Result<&Session, Failure>,
    _config: Option<&Config>,
) -> Check {
    check_with(
        environment,
        session,
        ENGINE_BUILT,
        &Published::new(environment),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_engine_absent_inventory_remains_injectable_without_filesystem_calls() {
        let published = Published::new(&Environment::default());
        assert!(matches!(
            published.receipt().unwrap(),
            Receipt::NonePublished
        ));
    }
}
