//! N11 concurrent admission, overflow and resume edge contracts.
use super::n11_support::{GIB, change, limits, reserve, resources};
use maestro_acquisition::transport::budget::{Pending, Usage};
use std::{
    sync::{Arc, Barrier},
    thread,
};

#[test]
fn n11_concurrent_admission_has_one_atomic_shared_ceiling() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| {
        snapshot.aggregate.cpu_millicores = 2000.try_into().unwrap();
    });
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let resources = resources.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                resources.reserve(
                    &[limits()],
                    Usage {
                        cpu_millicores: 2000,
                        ..Usage::default()
                    },
                )
            })
        })
        .collect();
    let outcomes: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Err(Pending::Cpu)))
            .count(),
        1
    );
    drop(outcomes);
    assert_eq!(resources.usage().unwrap(), Usage::default());
}

#[test]
fn n11_overflow_is_pending_not_wrapped_usage() {
    for (first, next) in [
        (
            Usage {
                cpu_millicores: u64::MAX,
                ..Usage::default()
            },
            Usage {
                cpu_millicores: 1,
                ..Usage::default()
            },
        ),
        (
            Usage {
                memory_bytes: u64::MAX,
                ..Usage::default()
            },
            Usage {
                memory_bytes: 1,
                ..Usage::default()
            },
        ),
        (
            Usage {
                staging_bytes: u64::MAX - 30 * GIB,
                ..Usage::default()
            },
            Usage {
                staging_bytes: 31 * GIB,
                ..Usage::default()
            },
        ),
        (
            Usage {
                gpu_bytes: u64::MAX - 2 * GIB,
                ..Usage::default()
            },
            Usage {
                gpu_bytes: 3 * GIB,
                ..Usage::default()
            },
        ),
        (
            Usage {
                gpu_batches: u64::MAX,
                ..Usage::default()
            },
            Usage {
                gpu_batches: 1,
                ..Usage::default()
            },
        ),
    ] {
        let (controls, resources) = resources();
        let mut bound = limits();
        bound.cpu_millicores = u64::MAX.try_into().unwrap();
        bound.memory_bytes = u64::MAX.try_into().unwrap();
        bound.staging_bytes = u64::MAX.try_into().unwrap();
        bound.gpu_bytes = u64::MAX;
        bound.gpu_batches = u64::MAX;
        change(&controls, |snapshot| {
            snapshot.aggregate = bound.clone();
            snapshot.per_run = bound.clone();
            snapshot.free_disk_bytes = u64::MAX;
            snapshot.free_gpu_bytes = u64::MAX;
        });
        let held = resources.reserve(&[bound.clone()], first).unwrap();
        assert_eq!(
            resources.reserve(&[bound], next).map(|_| ()),
            Err(Pending::Accounting)
        );
        drop(held);
        assert_eq!(resources.usage().unwrap(), Usage::default());
    }
}

#[test]
fn n11_checkpoint_tightens_reserve_and_clears_only_after_fresh_controls() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| snapshot.free_disk_bytes = 32 * GIB);
    let mut held = reserve(
        &resources,
        Usage {
            staging_bytes: GIB,
            ..Usage::default()
        },
    );
    let mut strict = limits();
    strict.free_reserve_bytes = (32 * GIB).try_into().unwrap();
    assert_eq!(
        held.checkpoint(
            &[strict],
            Usage {
                staging_bytes: GIB,
                ..Usage::default()
            }
        ),
        Err(Pending::DiskReserve)
    );
    assert_eq!(
        held.checkpoint(
            &[limits()],
            Usage {
                staging_bytes: GIB,
                ..Usage::default()
            }
        ),
        Err(Pending::DiskReserve)
    );
    assert_eq!(held.checkpoint(&[], Usage::default()), Err(Pending::Bounds));
    assert_eq!(held.pending().unwrap(), Some(Pending::Bounds));
    *controls.0.lock().unwrap() = Err(Pending::Grant);
    assert_eq!(
        held.checkpoint(&[limits()], Usage::default()),
        Err(Pending::Grant)
    );
    assert_eq!(held.pending().unwrap(), Some(Pending::Grant));
    assert_eq!(resources.usage().unwrap().staging_bytes, GIB);
}

#[test]
fn n11_disabled_and_incompatible_gpu_batch_bounds_hold() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| snapshot.free_gpu_bytes = GIB);
    assert_eq!(
        resources
            .reserve(
                &[limits()],
                Usage {
                    gpu_batches: 1,
                    ..Usage::default()
                }
            )
            .map(|_| ()),
        Err(Pending::Gpu)
    );
    change(&controls, |snapshot| {
        snapshot.free_gpu_bytes = 20 * GIB;
        snapshot.per_run.gpu_bytes = 0;
    });
    assert_eq!(
        resources
            .reserve(
                &[limits()],
                Usage {
                    gpu_batches: 1,
                    ..Usage::default()
                }
            )
            .map(|_| ()),
        Err(Pending::Gpu)
    );
    change(&controls, |snapshot| {
        snapshot.per_run.gpu_bytes = 8 * GIB;
        snapshot.aggregate.gpu_batches = 0;
    });
    assert_eq!(
        resources
            .reserve(
                &[limits()],
                Usage {
                    gpu_batches: 1,
                    ..Usage::default()
                }
            )
            .map(|_| ()),
        Err(Pending::GpuBatch)
    );
    assert!(resources.reserve(&[limits()], Usage::default()).is_ok());
}

#[test]
fn n11_shared_controls_never_loosen_during_the_process() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| {
        snapshot.aggregate.cpu_millicores = 1000.try_into().unwrap();
    });
    let held = reserve(&resources, Usage::default());
    change(&controls, |snapshot| {
        snapshot.aggregate.cpu_millicores = 3000.try_into().unwrap();
    });
    assert_eq!(
        resources
            .reserve(
                &[limits()],
                Usage {
                    cpu_millicores: 1001,
                    ..Usage::default()
                }
            )
            .map(|_| ()),
        Err(Pending::Cpu)
    );
    drop(held);
}

#[test]
fn n11_cancellation_releases_only_its_owned_reservation() {
    let (_, resources) = resources();
    let first = reserve(
        &resources,
        Usage {
            memory_bytes: GIB,
            ..Usage::default()
        },
    );
    let second = reserve(
        &resources,
        Usage {
            memory_bytes: 2 * GIB,
            ..Usage::default()
        },
    );
    drop(first);
    assert_eq!(resources.usage().unwrap().memory_bytes, 2 * GIB);
    drop(second);
    assert_eq!(resources.usage().unwrap(), Usage::default());
}

#[test]
fn n11_host_reserve_floors_apply_to_every_new_allocation() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| {
        snapshot.aggregate.free_reserve_bytes = (40 * GIB).try_into().unwrap();
        snapshot.aggregate.gpu_reserve_bytes = (4 * GIB).try_into().unwrap();
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
}

#[test]
fn s6t_cpu_only_zero_gpu_allocation_is_admitted() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| {
        snapshot.aggregate.gpu_bytes = 0;
        snapshot.per_run.gpu_bytes = 0;
    });
    let mut bound = limits();
    bound.gpu_bytes = 0;
    let reservation = resources.reserve(&[bound], Usage::default()).unwrap();
    drop(reservation);
    assert_eq!(resources.usage().unwrap(), Usage::default());
}
