//! Explicit frozen graph settings; no defaults or independent settings resolver.

use super::port::ProjectionError;
use maestro_kernel::artifact::Digest;

/// Runtime selection from `graph.engine`; native types never cross the facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionEngine {
    /// Disable reads and builds before any filesystem or kernel work.
    None,
    /// Select the qualified embedded backend when compiled with `engine`.
    Ladybug,
}

/// The approved D14 settings and opaque frozen non-resource lock identity.
/// S1's resolver supplies these values when S2/S3 activation shares a branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSettings {
    /// Explicit pool bytes, never zero/auto.
    pub(super) buffer_pool_size: u64,
    /// Explicit maximum native database size, not a filesystem quota.
    pub(super) max_db_size: u64,
    /// Explicit native query threads, not Cargo parallelism.
    pub(super) max_num_threads: u64,
    /// Complete frozen non-resource lock digest supplied by the caller.
    pub(super) frozen_lock: Digest,
}

impl EngineSettings {
    /// Admit explicit values using the approved D14 bounds (plan A0).
    /// No Pi graph-engine equivalent exists; these are spec bounds, not new defaults.
    ///
    /// # Errors
    /// Refuses pool outside 16 MiB–1 GiB, non-power-of-two size outside
    /// 16 MiB–1 TiB, or threads outside 1–64.
    pub fn new(
        buffer_pool_size: u64,
        max_db_size: u64,
        max_num_threads: u64,
        frozen_lock: Digest,
    ) -> Result<Self, ProjectionError> {
        if !(16 * 1024 * 1024..=1024 * 1024 * 1024).contains(&buffer_pool_size)
            || !(16 * 1024 * 1024..=1024 * 1024 * 1024 * 1024).contains(&max_db_size)
            || !max_db_size.is_power_of_two()
            || !(1..=64).contains(&max_num_threads)
        {
            return Err(ProjectionError::Invalid(
                "graphdb settings violate the approved D14 bounds".into(),
            ));
        }
        Ok(Self {
            buffer_pool_size,
            max_db_size,
            max_num_threads,
            frozen_lock,
        })
    }

    /// Canonical graph-settings/1 identity: tag followed by three big-endian u64 values.
    #[must_use]
    pub fn identity(&self) -> Digest {
        let mut bytes = b"graph-settings/1\0".to_vec();
        for value in [
            self.buffer_pool_size,
            self.max_db_size,
            self.max_num_threads,
        ] {
            bytes.extend(value.to_be_bytes());
        }
        Digest::of(&bytes)
    }

    /// The exact caller-owned frozen lock carried through builds, publication and reads.
    #[must_use]
    pub fn frozen_lock(&self) -> &Digest {
        &self.frozen_lock
    }
}
