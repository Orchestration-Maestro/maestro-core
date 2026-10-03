//! CLI health ports delegated to the opaque read-only knowledge probe.

use super::{
    check::{Check, Outcome},
    graph::check_with,
};
use crate::{
    cli::setup::graph::DIRECTORY,
    failure::Failure,
    settings::{GraphEngine, Session},
};
use maestro_filesystem::SystemFileLock;
use maestro_kernel::{
    paths::{self, Environment},
    scope::Config,
};
use maestro_knowledge::graph::projection::{
    EngineSettings,
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
    /// Explicit settings and complete identity from read-only catalog verification.
    settings: Option<EngineSettings>,
}
impl<'a> Published<'a> {
    /// Delay every receipt/filesystem/engine operation until health selects it.
    fn new(
        environment: &'a Environment,
        config: Option<&'a Config>,
        settings: Option<EngineSettings>,
    ) -> Self {
        Self {
            environment,
            config,
            settings,
        }
    }
}
impl PublishedGraph for Published<'_> {
    fn receipt(&self) -> Result<Receipt<'_>, ProbeError> {
        let data = paths::data_dir(self.environment).map_err(|_| ProbeError::AuthorityMissing)?;
        let config = self.config.ok_or(ProbeError::InventoryUnreadable)?;
        Ok(Probe::receipt(
            &data,
            &data.join(DIRECTORY),
            config,
            &SystemFileLock,
            self.settings.clone(),
        )?
        .into_receipt())
    }
}
/// Production graph check; the lazy adapter is never called while disabled.
pub(super) fn check(
    environment: &Environment,
    session: Result<&Session, Failure>,
    config: Option<&Config>,
) -> Check {
    let activation = session.as_ref().ok().and_then(|session| {
        if GraphEngine::from_session(session).ok() == Some(GraphEngine::Ladybug) {
            Some(session.graph_settings())
        } else {
            None
        }
    });
    let settings = activation
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned()
        .flatten();
    let mut check = check_with(
        environment,
        session,
        ENGINE_BUILT,
        &Published::new(environment, config, settings),
    );
    if let Some(activation) = activation {
        let note = match activation {
            Ok(_) => "settings identity not yet bound to published graph".to_owned(),
            Err(error) => format!("graph activation unavailable: {error}"),
        };
        match &mut check.outcome {
            Outcome::Passed(detail) | Outcome::NotChecked(detail) => {
                detail.push_str("; ");
                detail.push_str(&note);
            }
            Outcome::Failed { problem, next } => {
                if problem.contains("configured but not activated") {
                    *problem = note;
                    *next = "restore the admitted authoring lock and fix graphdb settings; \
                         run maestro init in a trusted workspace, then maestro doctor"
                        .into();
                }
            }
        }
    }
    check
}
