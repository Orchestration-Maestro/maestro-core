//! Engine-only adapter inputs with redacted root and opaque lock diagnostics.

use super::settings::EngineSettings;
use maestro_filesystem::FileLock;
use std::{any::type_name_of_val, fmt, path::PathBuf};

/// Adapter inputs separate from the factory/session types, avoiding an import cycle.
pub(super) struct ProjectionConfiguration<'a> {
    /// Configured owned graph root, never an engine filename.
    pub(super) path: PathBuf,
    /// Explicit frozen settings supplied by the application.
    pub(super) settings: EngineSettings,
    /// Lock primitive supplied by the application.
    pub(super) locks: &'a dyn FileLock,
}

impl fmt::Debug for ProjectionConfiguration<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionConfiguration")
            .field("settings", &self.settings)
            .field("locks", &type_name_of_val(self.locks))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::projection::{ProjectionEngine, ProjectionFactory};
    use maestro_filesystem::SystemFileLock;
    use maestro_kernel::artifact::Digest;

    #[test]
    fn configuration_and_factory_debug_redact_private_path() {
        let path = PathBuf::from("private-graph-root-marker");
        let settings =
            EngineSettings::new(16 * 1024 * 1024, 64 * 1024 * 1024, 1, Digest::of(b"lock"))
                .unwrap();
        let configuration = ProjectionConfiguration {
            path: path.clone(),
            settings: settings.clone(),
            locks: &SystemFileLock,
        };
        let factory =
            ProjectionFactory::new(&path, ProjectionEngine::Ladybug, settings, &SystemFileLock);
        for (debug, name) in [
            (format!("{configuration:?}"), "ProjectionConfiguration"),
            (format!("{factory:?}"), "ProjectionFactory"),
        ] {
            assert!(debug.starts_with(name));
            assert!(
                !debug.contains("private-graph-root-marker"),
                "private path leaked"
            );
        }
    }
}
