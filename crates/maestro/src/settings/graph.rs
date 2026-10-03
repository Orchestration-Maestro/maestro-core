//! The configured local graph engine.

use super::session::{GraphActivationError, Session};
use crate::failure::Failure;
#[cfg(any(feature = "engine", test))]
use maestro_kernel::artifact::Digest;
#[cfg(any(feature = "engine", test))]
use maestro_knowledge::graph::projection::EngineSettings;
use std::collections::BTreeSet;

/// The graph engine selected by `graph.engine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphEngine {
    /// Graph reads are disabled and make no engine calls.
    None,
    /// The in-process `LadybugDB` engine.
    Ladybug,
}

#[cfg(any(feature = "engine", test))]
impl Session {
    /// Native settings from the one admitted snapshot; disabled means no activation work.
    pub(crate) fn graph_settings(&self) -> Result<Option<EngineSettings>, Failure> {
        if GraphEngine::from_session(self)? == GraphEngine::None {
            return Ok(None);
        }
        if let Some(error) = &self.graph_activation_error {
            return Err(Failure::refused(error.to_string()));
        }
        let lock = self.frozen_lock.as_deref().ok_or_else(|| {
            Failure::refused(
                "graph settings have no admitted authoring lock; \
             run maestro init in a trusted workspace",
            )
        })?;
        let lock = lock.strip_prefix("sha256:").ok_or_else(|| {
            Failure::refused(
                "the admitted authoring lock identity is invalid; run maestro init again",
            )
        })?;
        let lock = Digest::parse(lock).map_err(|_| {
            Failure::refused(
                "the admitted authoring lock identity is invalid; run maestro init again",
            )
        })?;
        let resolved = self.catalog_resolved();
        let integer = |key| {
            resolved
                .integer(key)
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| {
                    Failure::refused(format!(
                        "{key}: invalid graph setting; fix it with maestro config"
                    ))
                })
        };
        EngineSettings::new(
            integer("graphdb.buffer_pool_size")?,
            integer("graphdb.max_db_size")?,
            integer("graphdb.max_num_threads")?,
            lock,
        )
        .map(Some)
        .map_err(|error| Failure::refused_by(&error))
    }
}

impl GraphEngine {
    /// The setting that selects it.
    pub(crate) const KEY: &str = "graph.engine";

    /// Reads the selected engine from the resolved settings.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when the registry lacks the setting or it holds
    /// another value, which the registry's choices rule out.
    pub(crate) fn from_session(session: &Session) -> Result<Self, Failure> {
        Self::read(session).map(|(engine, _)| engine)
    }

    /// Reads the selected engine and reports the setting key it read.
    pub(crate) fn read(session: &Session) -> Result<(Self, BTreeSet<String>), Failure> {
        if session.graph_activation_error == Some(GraphActivationError::EngineMissing) {
            return Ok((Self::Ladybug, BTreeSet::from([Self::KEY.to_owned()])));
        }
        let engine = match session.resolved().text(Self::KEY) {
            Some("none") => Self::None,
            Some("ladybug") => Self::Ladybug,
            _ => return Err(Failure::failed("graph.engine is missing or not a choice")),
        };
        Ok((engine, BTreeSet::from([Self::KEY.to_owned()])))
    }
}
