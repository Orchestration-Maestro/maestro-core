//! The limit values, one immutable [`Limits`].

use std::time::Duration;

/// One mebibyte, 1,048,576 bytes.
const MIB: u64 = 1_048_576;

/// The numeric bounds on every catalog input, fixed by plan D2. A larger
/// production need is a reviewed contract change, never a user, catalog or
/// command-line override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Bytes in one source or preference file.
    pub source_file_bytes: u64,
    /// Container levels in one source or preference document, the root
    /// counting as one.
    pub source_depth: usize,
    /// Resources in one catalog's sources.
    pub catalog_resources: usize,
    /// Bytes in one archive entry.
    pub archive_entry_bytes: u64,
    /// Bytes in the whole archive stream and in its entries' payload together.
    pub archive_total_bytes: u64,
    /// Entries in one archive, `bundle.json` included.
    pub archive_entries: usize,
    /// Container levels in the bundle manifest, the root counting as one.
    pub manifest_depth: usize,
    /// Bytes in one downloaded artifact.
    pub download_bytes: u64,
    /// Wall time of one download, redirects and retries included.
    pub download_time: Duration,
    /// Wall time of one verifier run.
    pub verifier_time: Duration,
    /// Bytes the verifier may write on stdout.
    pub verifier_stdout_bytes: u64,
    /// Bytes the verifier may write on stderr.
    pub verifier_stderr_bytes: u64,
    /// Wall time of one whole startup discovery attempt.
    pub startup_discovery_time: Duration,
}

impl Limits {
    /// The production limits, plan D2's table.
    pub const PRODUCTION: Self = Self {
        source_file_bytes: MIB,
        source_depth: 32,
        catalog_resources: 4_096,
        archive_entry_bytes: 16 * MIB,
        archive_total_bytes: 256 * MIB,
        archive_entries: 4_096,
        manifest_depth: 32,
        download_bytes: 256 * MIB,
        download_time: Duration::from_mins(1),
        verifier_time: Duration::from_secs(30),
        verifier_stdout_bytes: 16 * MIB,
        verifier_stderr_bytes: MIB,
        startup_discovery_time: Duration::from_secs(5),
    };
}
