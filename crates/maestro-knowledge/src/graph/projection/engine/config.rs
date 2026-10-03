//! The single native translation of caller-owned frozen graph settings.
use crate::graph::projection::settings::EngineSettings;
use lbug::SystemConfig;

/// Read-only versus writable is chosen by the lifecycle, never by user configuration.
pub(super) fn native(settings: &EngineSettings) -> SystemConfig {
    SystemConfig::default()
        .buffer_pool_size(settings.buffer_pool_size)
        .max_db_size(settings.max_db_size)
        .max_num_threads(settings.max_num_threads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_kernel::artifact::Digest;

    #[test]
    fn every_explicit_graphdb_value_reaches_the_native_config_unchanged() {
        let settings =
            EngineSettings::new(32 * 1024 * 1024, 128 * 1024 * 1024, 3, Digest::of(b"lock"))
                .unwrap();
        // The pinned dependency exposes Debug, not getters, for these native constructor fields.
        let actual = format!("{:?}", native(&settings));
        for field in [
            "buffer_pool_size: 33554432",
            "max_db_size: 134217728",
            "max_num_threads: 3",
        ] {
            assert!(actual.contains(field), "{actual}");
        }
    }
}
