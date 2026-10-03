//! Shared synthetic explicit N11 host/settings fixtures.
#![expect(
    clippy::indexing_slicing,
    reason = "synthetic fixture keys must fail loudly"
)]
use maestro_acquisition::{
    lifecycle::resources::{Reservation, ResourceControls, ResourceSnapshot, Resources},
    policy::limits::Limits,
    transport::budget::{Pending, Usage},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Integer GiB, independently pinned by these synthetic cases.
pub(super) const GIB: u64 = 1 << 30;

/// Explicit complete bounds, with OA3's resource envelope.
pub(super) fn limits() -> Limits {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/policy.json")).unwrap();
    let mut value = fixture["aggregate_limits"].clone();
    for (field, amount) in [
        ("cpu_millicores", 3000),
        ("memory_bytes", 8 * GIB),
        ("staging_bytes", 20 * GIB),
        ("free_reserve_bytes", 30 * GIB),
        ("gpu_reserve_bytes", 2 * GIB),
        ("gpu_bytes", 8 * GIB),
        ("gpu_batches", 1),
        ("source_runs", 4),
    ] {
        value[field] = json!(amount);
    }
    serde_json::from_value(value).unwrap()
}

/// Replaceable current host/grant measurements, never source-supplied authority.
#[derive(Debug)]
pub(super) struct Controls(pub(super) Mutex<Result<ResourceSnapshot, Pending>>);
impl ResourceControls for Controls {
    fn snapshot(&self) -> Result<ResourceSnapshot, Pending> {
        self.0.lock().unwrap().clone()
    }
}

/// A fresh synthetic host with sufficient disk and VRAM.
pub(super) fn resources() -> (Arc<Controls>, Resources) {
    let mut per_run = limits();
    per_run.cpu_millicores = 2000.try_into().unwrap();
    per_run.memory_bytes = (4 * GIB).try_into().unwrap();
    per_run.staging_bytes = (10 * GIB).try_into().unwrap();
    let controls = Arc::new(Controls(Mutex::new(Ok(ResourceSnapshot {
        aggregate: limits(),
        per_run,
        free_disk_bytes: 100 * GIB,
        free_gpu_bytes: 12 * GIB,
        interactive_pending: false,
    }))));
    (controls.clone(), Resources::new(controls))
}

/// Change fresh controls without giving policy authority to the caller.
pub(super) fn change(controls: &Controls, edit: impl FnOnce(&mut ResourceSnapshot)) {
    edit(controls.0.lock().unwrap().as_mut().unwrap());
}

/// Reserve a small independent run.
pub(super) fn reserve(resources: &Resources, usage: Usage) -> Reservation {
    let result = resources.reserve(&[limits()], usage);
    assert!(result.is_ok(), "{result:?}");
    result.unwrap()
}
