//! Plan D2's production constants, asserted once here; every boundary test
//! injects small limits instead of giant inputs.

use super::Limits;
use std::time::Duration;

#[test]
fn production_limits_are_plan_d2_table() {
    let mebibyte = 1_048_576;
    assert_eq!(
        Limits::PRODUCTION,
        Limits {
            source_file_bytes: mebibyte,
            source_depth: 32,
            catalog_resources: 4_096,
            archive_entry_bytes: 16 * mebibyte,
            archive_total_bytes: 256 * mebibyte,
            archive_entries: 4_096,
            manifest_depth: 32,
            download_bytes: 256 * mebibyte,
            download_time: Duration::from_secs(60),
            verifier_time: Duration::from_secs(30),
            verifier_stdout_bytes: 16 * mebibyte,
            verifier_stderr_bytes: mebibyte,
            startup_discovery_time: Duration::from_secs(5),
        }
    );
}
