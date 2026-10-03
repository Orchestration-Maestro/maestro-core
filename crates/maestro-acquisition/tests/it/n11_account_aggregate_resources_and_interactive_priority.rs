//! Synthetic N11 accounting contracts; no processes, live grants or GPU jobs.
use super::n11_support::{GIB, change, limits, reserve, resources};
use maestro_acquisition::{
    lifecycle::resources::{NoGrantControls, Resources},
    policy::limits::Limits,
    transport::budget::{Pending, Usage, compose},
};
use serde_json::json;
use std::sync::Arc;

#[test]
fn n11_field_kinds_compose_every_field_in_both_orders() {
    let original = serde_json::to_value(limits()).unwrap();
    for (field, value) in original.as_object().unwrap() {
        if field == "decode" {
            continue;
        }
        let mut low = original.clone();
        let mut high = original.clone();
        low[field] = json!(1);
        high[field] = json!(2);
        let low: Limits = serde_json::from_value(low).unwrap();
        let high: Limits = serde_json::from_value(high).unwrap();
        let floor = [
            "free_reserve_bytes",
            "gpu_reserve_bytes",
            "origin_interval_ms",
        ]
        .contains(&field.as_str());
        for inputs in [[low.clone(), high.clone()], [high.clone(), low.clone()]] {
            let got = serde_json::to_value(compose(&inputs).unwrap()).unwrap();
            assert_eq!(
                got[field],
                json!(if floor { 2 } else { 1 }),
                "{field}: {value}"
            );
        }
    }
    for field in [
        "expanded_bytes",
        "expansion_ratio",
        "nested_levels",
        "members",
        "decoded_pixels",
        "elapsed_ms",
        "memory_bytes",
    ] {
        let mut low = original.clone();
        let mut high = original.clone();
        low["decode"][field] = json!(1);
        high["decode"][field] = json!(2);
        let low: Limits = serde_json::from_value(low).unwrap();
        let high: Limits = serde_json::from_value(high).unwrap();
        for inputs in [[low.clone(), high.clone()], [high.clone(), low.clone()]] {
            let got = serde_json::to_value(compose(&inputs).unwrap()).unwrap();
            assert_eq!(got["decode"][field], json!(1), "decode.{field}");
        }
    }
    assert_eq!(compose(&[]), Err(Pending::Bounds));
}

#[test]
fn n11_mixed_reserves_and_caps_have_no_precedence() {
    let mut weaker = limits();
    weaker.free_reserve_bytes = (10 * GIB).try_into().unwrap();
    weaker.gpu_reserve_bytes = GIB.try_into().unwrap();
    weaker.staging_bytes = (10 * GIB).try_into().unwrap();
    for inputs in [[limits(), weaker.clone()], [weaker, limits()]] {
        let effective = compose(&inputs).unwrap();
        assert_eq!(effective.free_reserve_bytes.get(), 30 * GIB);
        assert_eq!(effective.staging_bytes.get(), 10 * GIB);
        assert_eq!(effective.gpu_reserve_bytes.get(), 2 * GIB);
    }
}

#[test]
fn n11_combined_usage_and_run_count_cannot_multiply_ceilings() {
    for (first, next, reason) in [
        (
            Usage {
                cpu_millicores: 2000,
                ..Usage::default()
            },
            Usage {
                cpu_millicores: 1001,
                ..Usage::default()
            },
            Pending::Cpu,
        ),
        (
            Usage {
                memory_bytes: 4 * GIB,
                ..Usage::default()
            },
            Usage {
                memory_bytes: 4 * GIB,
                ..Usage::default()
            },
            Pending::Memory,
        ),
        (
            Usage {
                staging_bytes: 10 * GIB,
                ..Usage::default()
            },
            Usage {
                staging_bytes: 10 * GIB,
                ..Usage::default()
            },
            Pending::Staging,
        ),
        (
            Usage {
                gpu_bytes: 5 * GIB,
                ..Usage::default()
            },
            Usage {
                gpu_bytes: 4 * GIB,
                ..Usage::default()
            },
            Pending::Gpu,
        ),
        (
            Usage {
                gpu_batches: 1,
                ..Usage::default()
            },
            Usage {
                gpu_batches: 1,
                ..Usage::default()
            },
            Pending::GpuBatch,
        ),
    ] {
        let (controls, resources) = resources();
        // Tighten aggregates so both individual requests fit, but their sum does not.
        change(&controls, |snapshot| {
            snapshot.aggregate.memory_bytes = (7 * GIB).try_into().unwrap();
            snapshot.aggregate.staging_bytes = (19 * GIB).try_into().unwrap();
        });
        let held = reserve(&resources, first);
        assert_eq!(
            resources.reserve(&[limits()], next).map(|_| ()),
            Err(reason)
        );
        assert_eq!(resources.usage().unwrap(), first);
        drop(held);
        assert_eq!(resources.usage().unwrap(), Usage::default());
        assert!(resources.reserve(&[limits()], next).is_ok());
    }
    let (_, resources) = resources();
    let held: Vec<_> = (0..4)
        .map(|_| reserve(&resources, Usage::default()))
        .collect();
    assert_eq!(
        resources.reserve(&[limits()], Usage::default()).map(|_| ()),
        Err(Pending::Runs)
    );
    drop(held);
    assert!(resources.reserve(&[limits()], Usage::default()).is_ok());
}

#[test]
fn n11_per_run_limits_and_missing_controls_hold() {
    let (_, resources) = resources();
    for (usage, reason) in [
        (
            Usage {
                cpu_millicores: 2001,
                ..Usage::default()
            },
            Pending::Cpu,
        ),
        (
            Usage {
                memory_bytes: 4 * GIB + 1,
                ..Usage::default()
            },
            Pending::Memory,
        ),
        (
            Usage {
                staging_bytes: 10 * GIB + 1,
                ..Usage::default()
            },
            Pending::Staging,
        ),
        (
            Usage {
                gpu_bytes: 8 * GIB + 1,
                ..Usage::default()
            },
            Pending::Gpu,
        ),
        (
            Usage {
                gpu_batches: 2,
                ..Usage::default()
            },
            Pending::GpuBatch,
        ),
    ] {
        assert_eq!(
            resources.reserve(&[limits()], usage).map(|_| ()),
            Err(reason)
        );
    }
    assert_eq!(
        resources.reserve(&[], Usage::default()).map(|_| ()),
        Err(Pending::Bounds)
    );
    let resources = Resources::new(Arc::new(NoGrantControls));
    assert_eq!(
        resources.reserve(&[limits()], Usage::default()).map(|_| ()),
        Err(Pending::Grant)
    );
}

#[test]
fn n11_streaming_pauses_before_disk_reserve_and_reports_pending() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| snapshot.free_disk_bytes = 32 * GIB);
    let mut held = reserve(
        &resources,
        Usage {
            staging_bytes: GIB,
            ..Usage::default()
        },
    );
    assert_eq!(
        held.checkpoint(
            &[limits()],
            Usage {
                staging_bytes: 2 * GIB,
                ..Usage::default()
            },
        ),
        Ok(())
    );
    let before = resources.usage().unwrap();
    assert_eq!(
        held.checkpoint(
            &[limits()],
            Usage {
                staging_bytes: 2 * GIB + 1,
                ..Usage::default()
            }
        ),
        Err(Pending::DiskReserve)
    );
    assert_eq!(held.pending().unwrap(), Some(Pending::DiskReserve));
    assert_eq!(resources.usage().unwrap(), before);
    change(&controls, |snapshot| snapshot.free_disk_bytes = 40 * GIB);
    assert_eq!(
        held.checkpoint(
            &[limits()],
            Usage {
                staging_bytes: 3 * GIB,
                ..Usage::default()
            },
        ),
        Ok(())
    );
    assert_eq!(held.pending().unwrap(), None);
    // Fresh disk loss also pauses without allocating a new byte.
    change(&controls, |snapshot| {
        snapshot.free_disk_bytes = 30 * GIB - 1;
    });
    assert_eq!(
        held.checkpoint(&[limits()], Usage::default()),
        Err(Pending::DiskReserve)
    );
}

#[test]
fn n11_gpu_headroom_clamps_and_zero_gpu_disables_work() {
    for (free, bytes, expected) in [
        (GIB, 0, Ok(())),
        (GIB, 1, Err(Pending::Gpu)),
        (3 * GIB, GIB, Ok(())),
        (3 * GIB, GIB + 1, Err(Pending::Gpu)),
        (20 * GIB, 8 * GIB, Ok(())),
        (20 * GIB, 8 * GIB + 1, Err(Pending::Gpu)),
    ] {
        let (controls, resources) = resources();
        change(&controls, |snapshot| snapshot.free_gpu_bytes = free);
        assert_eq!(
            resources
                .reserve(
                    &[limits()],
                    Usage {
                        gpu_bytes: bytes,
                        ..Usage::default()
                    }
                )
                .map(|_| ()),
            expected
        );
    }
    let (controls, resources) = resources();
    change(&controls, |snapshot| snapshot.aggregate.gpu_bytes = 0);
    for usage in [
        Usage {
            gpu_bytes: 1,
            ..Usage::default()
        },
        Usage {
            gpu_batches: 1,
            ..Usage::default()
        },
    ] {
        assert_eq!(
            resources.reserve(&[limits()], usage).map(|_| ()),
            Err(Pending::Gpu)
        );
    }
}

#[test]
fn n11_interactive_priority_checkpoints_without_eviction() {
    let (controls, resources) = resources();
    let batch = Usage {
        gpu_bytes: GIB,
        gpu_batches: 1,
        ..Usage::default()
    };
    let mut held = reserve(&resources, batch);
    change(&controls, |snapshot| {
        snapshot.interactive_pending = true;
        snapshot.free_gpu_bytes = 3 * GIB;
    });
    assert_eq!(
        held.checkpoint(&[limits()], batch),
        Err(Pending::Interactive)
    );
    assert_eq!(held.pending().unwrap(), Some(Pending::Interactive));
    assert_eq!(
        resources.reserve(&[limits()], Usage::default()).map(|_| ()),
        Err(Pending::Interactive)
    );
    assert_eq!(resources.usage().unwrap(), batch);
    // The owner stops its batch, then releases only its own allocation.
    drop(held);
    assert_eq!(resources.usage().unwrap(), Usage::default());
    change(&controls, |snapshot| snapshot.interactive_pending = false);
    assert!(resources.reserve(&[limits()], batch).is_ok());
}

#[test]
fn n11_resume_tightening_is_monotonic_and_covers_live_aggregate() {
    let (controls, resources) = resources();
    let usage = Usage {
        cpu_millicores: 1000,
        staging_bytes: GIB,
        ..Usage::default()
    };
    let mut held = reserve(&resources, usage);
    let mut tight = limits();
    tight.cpu_millicores = 500.try_into().unwrap();
    assert_eq!(held.checkpoint(&[tight], usage), Err(Pending::Cpu));
    assert_eq!(held.checkpoint(&[limits()], usage), Err(Pending::Cpu));
    assert_eq!(
        held.checkpoint(
            &[limits()],
            Usage {
                cpu_millicores: 500,
                staging_bytes: GIB,
                ..Usage::default()
            },
        ),
        Ok(())
    );
    change(&controls, |snapshot| {
        snapshot.aggregate.staging_bytes = 1.try_into().unwrap();
    });
    assert_eq!(held.checkpoint(&[limits()], usage), Err(Pending::Cpu));
    assert_eq!(
        held.checkpoint(
            &[limits()],
            Usage {
                cpu_millicores: 500,
                staging_bytes: GIB,
                ..Usage::default()
            }
        ),
        Err(Pending::Staging)
    );
}

#[test]
fn n11_active_floors_remain_shared_and_restart_remeasures() {
    let (controls, resources) = resources();
    let mut strict = limits();
    strict.free_reserve_bytes = (40 * GIB).try_into().unwrap();
    strict.gpu_reserve_bytes = (4 * GIB).try_into().unwrap();
    let held = resources.reserve(&[strict], Usage::default()).unwrap();
    change(&controls, |snapshot| {
        snapshot.free_disk_bytes = 40 * GIB;
        snapshot.free_gpu_bytes = 4 * GIB;
    });
    assert_eq!(
        resources
            .reserve(
                &[limits()],
                Usage {
                    staging_bytes: 1,
                    ..Usage::default()
                }
            )
            .map(|_| ()),
        Err(Pending::DiskReserve)
    );
    assert_eq!(
        resources
            .reserve(
                &[limits()],
                Usage {
                    gpu_bytes: 1,
                    ..Usage::default()
                }
            )
            .map(|_| ()),
        Err(Pending::Gpu)
    );
    drop(held);
    assert!(
        resources
            .reserve(
                &[limits()],
                Usage {
                    staging_bytes: 1,
                    ..Usage::default()
                }
            )
            .is_ok()
    );
    let restarted = Resources::new(controls.clone());
    change(&controls, |snapshot| {
        snapshot.free_disk_bytes = 29 * GIB;
        snapshot.free_gpu_bytes = GIB;
    });
    assert_eq!(
        restarted.reserve(&[limits()], Usage::default()).map(|_| ()),
        Err(Pending::DiskReserve)
    );
    change(&controls, |snapshot| snapshot.free_disk_bytes = 100 * GIB);
    assert_eq!(
        restarted
            .reserve(
                &[limits()],
                Usage {
                    gpu_bytes: 1,
                    ..Usage::default()
                }
            )
            .map(|_| ()),
        Err(Pending::Gpu)
    );
    *controls.0.lock().unwrap() = Err(Pending::Unenforceable);
    assert_eq!(
        restarted.reserve(&[limits()], Usage::default()).map(|_| ()),
        Err(Pending::Unenforceable)
    );
}
