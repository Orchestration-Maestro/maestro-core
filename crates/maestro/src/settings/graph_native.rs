//! Native graph settings from the admitted session snapshot.

use super::{GraphEngine, Session};
use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::graph::projection::EngineSettings;

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
