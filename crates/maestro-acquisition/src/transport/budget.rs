//! Field-kind composition and typed resource-induced pending work.
use crate::policy::limits::{DecodeLimits, Limits};

/// Why ingestion must checkpoint/hold rather than allocate resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// No complete explicit envelope was supplied.
    Bounds,
    /// No current acquisition grant bounds exist.
    Grant,
    /// An adapter cannot enforce or measure the requested controls.
    Unenforceable,
    /// Counter overflow, unavailable ownership or poisoned accounting state.
    Accounting,
    /// Interactive work has priority at the next safe checkpoint.
    Interactive,
    /// Concurrent source-run ceiling.
    Runs,
    /// Owned CPU allocation ceiling.
    Cpu,
    /// Owned RAM allocation ceiling.
    Memory,
    /// Temporary/staging allocation ceiling.
    Staging,
    /// GPU ceiling or insufficient measured VRAM headroom.
    Gpu,
    /// Concurrent ingestion GPU-batch ceiling.
    GpuBatch,
    /// Free backing-store reserve would be crossed.
    DiskReserve,
}

/// Currently owned allocations; supplied before starting/growing owned work.
///
/// Zero means no allocation, not an absent or unbounded control. These counters
/// are not CPU/VRAM measurements and never authorize a process or GPU job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// CPU milli-cores reserved for owned work.
    pub cpu_millicores: u64,
    /// RAM bytes reserved for owned work.
    pub memory_bytes: u64,
    /// Owned staging bytes, including admitted writes not yet materialized.
    pub staging_bytes: u64,
    /// VRAM bytes reserved for owned ingestion work.
    pub gpu_bytes: u64,
    /// Owned ingestion batches, independent of VRAM bytes.
    pub gpu_batches: u64,
}

/// Compose every explicit bound by its field kind, never precedence.
///
/// # Errors
/// Empty inputs hold rather than selecting absent/unbounded defaults.
pub fn compose(bounds: &[Limits]) -> Result<Limits, Pending> {
    let mut effective = bounds.first().cloned().ok_or(Pending::Bounds)?;
    for bound in bounds.iter().skip(1) {
        tighten(&mut effective, bound);
    }
    Ok(effective)
}

/// Retain every stricter numeric field and the exact disabled-entities invariant.
pub(crate) fn tighten(effective: &mut Limits, bound: &Limits) {
    effective.requests = effective.requests.min(bound.requests);
    effective.pages = effective.pages.min(bound.pages);
    effective.partitions = effective.partitions.min(bound.partitions);
    effective.redirects = effective.redirects.min(bound.redirects);
    effective.depth = effective.depth.min(bound.depth);
    effective.elapsed_ms = effective.elapsed_ms.min(bound.elapsed_ms);
    effective.wire_bytes = effective.wire_bytes.min(bound.wire_bytes);
    effective.dom_bytes = effective.dom_bytes.min(bound.dom_bytes);
    effective.asset_bytes = effective.asset_bytes.min(bound.asset_bytes);
    effective.staging_bytes = effective.staging_bytes.min(bound.staging_bytes);
    effective.cpu_millicores = effective.cpu_millicores.min(bound.cpu_millicores);
    effective.memory_bytes = effective.memory_bytes.min(bound.memory_bytes);
    effective.source_runs = effective.source_runs.min(bound.source_runs);
    effective.origin_concurrency = effective.origin_concurrency.min(bound.origin_concurrency);
    effective.retries = effective.retries.min(bound.retries);
    effective.max_backoff_ms = effective.max_backoff_ms.min(bound.max_backoff_ms);
    effective.gpu_batches = effective.gpu_batches.min(bound.gpu_batches);
    effective.gpu_bytes = effective.gpu_bytes.min(bound.gpu_bytes);
    effective.free_reserve_bytes = effective.free_reserve_bytes.max(bound.free_reserve_bytes);
    effective.gpu_reserve_bytes = effective.gpu_reserve_bytes.max(bound.gpu_reserve_bytes);
    effective.origin_interval_ms = effective.origin_interval_ms.max(bound.origin_interval_ms);
    tighten_decode(&mut effective.decode, &bound.decode);
}

/// Decode ceilings compose identically; XML entities are always disabled.
fn tighten_decode(effective: &mut DecodeLimits, bound: &DecodeLimits) {
    effective.expanded_bytes = effective.expanded_bytes.min(bound.expanded_bytes);
    effective.expansion_ratio = effective.expansion_ratio.min(bound.expansion_ratio);
    effective.nested_levels = effective.nested_levels.min(bound.nested_levels);
    effective.members = effective.members.min(bound.members);
    effective.decoded_pixels = effective.decoded_pixels.min(bound.decoded_pixels);
    effective.elapsed_ms = effective.elapsed_ms.min(bound.elapsed_ms);
    effective.memory_bytes = effective.memory_bytes.min(bound.memory_bytes);
}
