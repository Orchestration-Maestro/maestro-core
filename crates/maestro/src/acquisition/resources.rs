//! OA3 bounds are data; fresh backing-store measurements never come from bindings.
use crate::failure::Failure;
#[cfg(target_os = "linux")]
use maestro_acquisition::lifecycle::resources::{ResourceControls, ResourceSnapshot};
#[cfg(any(target_os = "linux", test))]
use maestro_acquisition::{
    policy::{
        limits::{DecodeLimits, Limits},
        resolve::parse_resource,
    },
    transport::budget::{Pending, compose},
};
#[cfg(any(target_os = "linux", test))]
use serde::Deserialize;
#[cfg(any(target_os = "linux", test))]
use std::num::NonZeroU64;
#[cfg(target_os = "linux")]
use {rustix::fs::statvfs, std::path::PathBuf};

/// Only the embedded, strict OA3 wire version can supply operational bounds.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[cfg(any(target_os = "linux", test))]
enum Version {
    /// Approved operational defaults, frozen on 2026-09-30.
    #[serde(rename = "maestro-acquisition-oa3/1")]
    V1,
}
/// Exact approved OA3 operational envelope, not a grant or capacity measurement.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(any(target_os = "linux", test))]
struct Envelope {
    /// Only the pinned envelope version is accepted.
    #[serde(rename = "schema")]
    _schema: Version,
    /// Aggregate source owners.
    source_runs: NonZeroU64,
    /// One in-flight request per origin.
    origin_concurrency: NonZeroU64,
    /// Aggregate minimum origin interval.
    origin_interval_ms: NonZeroU64,
    /// Finite retry ceiling.
    retries: u64,
    /// Longer server delay holds.
    max_backoff_ms: u64,
    /// Aggregate CPU ownership.
    aggregate_cpu_millicores: NonZeroU64,
    /// Per-run CPU ownership.
    run_cpu_millicores: NonZeroU64,
    /// Aggregate memory ownership.
    aggregate_memory_bytes: NonZeroU64,
    /// Per-run memory ownership.
    run_memory_bytes: NonZeroU64,
    /// Aggregate temporary storage.
    aggregate_staging_bytes: NonZeroU64,
    /// Per-run temporary storage.
    run_staging_bytes: NonZeroU64,
    /// Protected backing-store floor.
    free_reserve_bytes: NonZeroU64,
    /// Concurrent ingestion GPU batches, unused by public capture.
    gpu_batches: u64,
    /// GPU ceiling, unused by public capture.
    gpu_bytes: u64,
    /// Protected GPU floor, unused by public capture.
    gpu_reserve_bytes: NonZeroU64,
    /// Per-document decoding envelope.
    decode: DecodeLimits,
}
/// Compose ceilings with min and protected floors with max through N11.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn bounds(policy: &Limits, aggregate: bool) -> Result<Limits, Pending> {
    let envelope: Envelope =
        parse_resource(include_bytes!("oa3.json")).map_err(|_| Pending::Bounds)?;
    let mut ceiling = policy.clone();
    ceiling.source_runs = envelope.source_runs;
    ceiling.origin_concurrency = envelope.origin_concurrency;
    ceiling.origin_interval_ms = envelope.origin_interval_ms;
    ceiling.retries = envelope.retries;
    ceiling.max_backoff_ms = envelope.max_backoff_ms;
    ceiling.free_reserve_bytes = envelope.free_reserve_bytes;
    ceiling.gpu_reserve_bytes = envelope.gpu_reserve_bytes;
    ceiling.gpu_batches = envelope.gpu_batches;
    ceiling.gpu_bytes = envelope.gpu_bytes;
    ceiling.decode = envelope.decode;
    (
        ceiling.cpu_millicores,
        ceiling.memory_bytes,
        ceiling.staging_bytes,
    ) = if aggregate {
        (
            envelope.aggregate_cpu_millicores,
            envelope.aggregate_memory_bytes,
            envelope.aggregate_staging_bytes,
        )
    } else {
        (
            envelope.run_cpu_millicores,
            envelope.run_memory_bytes,
            envelope.run_staging_bytes,
        )
    };
    compose(&[policy.clone(), ceiling])
}
/// Live public capture requires supported fresh resource controls before any start.
pub(crate) fn supported(platform: &str) -> Result<(), Failure> {
    if platform != "linux" {
        return Err(Failure::refused(
            "live resource control unsupported on this host",
        ));
    }
    Ok(())
}
/// Production resource port, measuring the actual backing store at every checkpoint.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(crate) struct Host {
    /// Actual kernel storage root, supplied outside portable manifests.
    pub(crate) root: PathBuf,
    /// Checked collection envelope, never a supplied free-space number.
    pub(crate) policy: Limits,
}
#[cfg(target_os = "linux")]
impl ResourceControls for Host {
    fn snapshot(&self) -> Result<ResourceSnapshot, Pending> {
        let filesystem = statvfs(&self.root).map_err(|_| Pending::Unenforceable)?;
        let free_disk_bytes = filesystem
            .f_bavail
            .checked_mul(filesystem.f_frsize)
            .ok_or(Pending::Accounting)?;
        Ok(ResourceSnapshot {
            aggregate: bounds(&self.policy, true)?,
            per_run: bounds(&self.policy, false)?,
            free_disk_bytes,
            free_gpu_bytes: 0,
            interactive_pending: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Envelope, parse_resource};
    use serde_json::{Value, json};
    #[test]
    fn n14_oa3_envelope_pins_every_approved_operational_value() {
        // specs/006-native-acquisition/spec.md: Approved operational defaults (OA3, 2026-09-30).
        let bytes = include_bytes!("oa3.json");
        let envelope: Envelope = parse_resource(bytes).unwrap();
        assert_eq!(envelope.source_runs.get(), 4);
        let mut wrong: Value = serde_json::from_slice(bytes).unwrap();
        wrong["schema"] = json!("unsupported/0");
        assert!(parse_resource::<Envelope>(&serde_json::to_vec(&wrong).unwrap()).is_err());
        let wire: Value = serde_json::from_slice(bytes).unwrap();
        for (field, value) in [
            ("source_runs", 4_u64),
            ("origin_concurrency", 1),
            ("origin_interval_ms", 1000),
            ("retries", 2),
            ("max_backoff_ms", 60000),
            ("aggregate_cpu_millicores", 3000),
            ("run_cpu_millicores", 2000),
            ("aggregate_memory_bytes", 8_589_934_592),
            ("run_memory_bytes", 4_294_967_296),
            ("aggregate_staging_bytes", 21_474_836_480),
            ("run_staging_bytes", 10_737_418_240),
            ("free_reserve_bytes", 32_212_254_720),
            ("gpu_batches", 1),
            ("gpu_bytes", 8_589_934_592),
            ("gpu_reserve_bytes", 2_147_483_648),
        ] {
            assert_eq!(wire[field], json!(value), "{field}");
        }
        assert_eq!(
            wire["decode"],
            json!({
                "expanded_bytes":268_435_456, "expansion_ratio":100,
                "nested_levels":4, "members":1000, "decoded_pixels":32_000_000,
                "elapsed_ms":120_000, "memory_bytes":2_147_483_648_u64,
                "xml_entities":"disabled"
            })
        );
    }
}
