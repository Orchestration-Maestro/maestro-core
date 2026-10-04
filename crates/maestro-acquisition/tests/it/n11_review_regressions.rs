//! N11 review regressions for composed ceilings and retained allocation ownership.
use super::n11_support::{GIB, change, limits, reserve, resources};
use maestro_acquisition::transport::budget::{Pending, Usage};

#[test]
fn n11_review_source_run_bound() {
    let (_, resources) = resources();
    let mut bounds = limits();
    bounds.source_runs = 1.try_into().unwrap();
    let first = resources.reserve(&[bounds.clone()], Usage::default());
    println!(
        "source-run bound: first={:?}",
        first.as_ref().map(|_| ()).map_err(|pending| *pending)
    );
    let held = first.unwrap();
    let second = resources.reserve(&[bounds], Usage::default()).map(|_| ());
    println!(
        "source-run bound: second={second:?}, usage={:?}, pending={:?}",
        resources.usage().unwrap(),
        held.pending().unwrap()
    );
    assert_eq!(second, Err(Pending::Runs));
}

#[test]
fn n11_review_per_run_source_run_bound() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| {
        snapshot.per_run.source_runs = 1.try_into().unwrap();
    });
    let snapshot = controls.0.lock().unwrap().clone().unwrap();
    println!(
        "per-run source-run bound: aggregate={}, per_run={}",
        snapshot.aggregate.source_runs.get(),
        snapshot.per_run.source_runs.get()
    );
    let first = resources.reserve(&[limits()], Usage::default());
    println!(
        "per-run source-run bound: first={:?}",
        first.as_ref().map(|_| ()).map_err(|pending| *pending)
    );
    let held = first.unwrap();
    let second = resources.reserve(&[limits()], Usage::default()).map(|_| ());
    println!(
        "per-run source-run bound: second={second:?}, usage={:?}, pending={:?}",
        resources.usage().unwrap(),
        held.pending().unwrap()
    );
    assert_eq!(second, Err(Pending::Runs));
}

#[test]
fn n11_review_tightening_through_unavailable_controls() {
    let (controls, resources) = resources();
    let original = controls.0.lock().unwrap().clone().unwrap();
    let usage = Usage {
        cpu_millicores: 1000,
        ..Usage::default()
    };
    let mut held = reserve(&resources, usage);
    println!(
        "unavailable controls: reserved usage={:?}, pending={:?}",
        resources.usage().unwrap(),
        held.pending().unwrap()
    );
    *controls.0.lock().unwrap() = Err(Pending::Grant);
    let mut tight = limits();
    tight.cpu_millicores = 500.try_into().unwrap();
    let unavailable = held.checkpoint(&[tight], usage);
    println!(
        "unavailable controls: tightened={unavailable:?}, usage={:?}, pending={:?}",
        resources.usage().unwrap(),
        held.pending().unwrap()
    );
    assert_eq!(unavailable, Err(Pending::Grant));
    assert_eq!(held.pending().unwrap(), Some(Pending::Grant));
    assert_eq!(resources.usage().unwrap(), usage);
    assert_eq!(
        held.checkpoint(&[limits()], Usage::default()),
        Err(Pending::Grant)
    );
    assert_eq!(resources.usage().unwrap(), usage);
    *controls.0.lock().unwrap() = Ok(original);
    let restored = held.checkpoint(&[limits()], usage);
    println!(
        "unavailable controls: restored={restored:?}, usage={:?}, pending={:?}",
        resources.usage().unwrap(),
        held.pending().unwrap()
    );
    assert_eq!(restored, Err(Pending::Cpu));
}

#[test]
fn n11_retained_run_ceiling_applies_to_admission_and_checkpoints() {
    let (_, resources) = resources();
    let mut bounds = limits();
    bounds.source_runs = 1.try_into().unwrap();
    let mut first = reserve(&resources, Usage::default());
    let second = reserve(&resources, Usage::default());
    assert_eq!(
        first.checkpoint(&[bounds], Usage::default()),
        Err(Pending::Runs)
    );
    assert_eq!(first.pending().unwrap(), Some(Pending::Runs));
    assert_eq!(
        resources.reserve(&[limits()], Usage::default()).map(|_| ()),
        Err(Pending::Runs)
    );
    assert_eq!(
        first.checkpoint(&[limits()], Usage::default()),
        Err(Pending::Runs)
    );
    drop(second);
    assert_eq!(first.checkpoint(&[limits()], Usage::default()), Ok(()));
    assert_eq!(first.pending().unwrap(), None);
    assert_eq!(
        resources.reserve(&[limits()], Usage::default()).map(|_| ()),
        Err(Pending::Runs)
    );
    drop(first);
    assert!(resources.reserve(&[limits()], Usage::default()).is_ok());
}

#[test]
fn n11_review_checkpoint_records_replacement_usage() {
    let (_, resources) = resources();
    let mut held = reserve(&resources, Usage::default());
    let replacement = Usage {
        cpu_millicores: 1000,
        memory_bytes: GIB,
        staging_bytes: GIB,
        gpu_bytes: GIB,
        gpu_batches: 1,
    };
    let checkpoint = held.checkpoint(&[limits()], replacement);
    let owned = resources.usage().unwrap();
    println!("replacement usage: checkpoint={checkpoint:?}, usage={owned:?}");
    assert_eq!(checkpoint, Ok(()));
    assert_eq!(owned, replacement);
}

#[test]
fn n11_review_per_run_gpu_byte_ceiling() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| snapshot.per_run.gpu_bytes = GIB);
    let result = resources
        .reserve(
            &[limits()],
            Usage {
                gpu_bytes: GIB + 1,
                ..Usage::default()
            },
        )
        .map(|_| ());
    println!("per-run GPU byte ceiling: result={result:?}");
    assert_eq!(result, Err(Pending::Gpu));
}

#[test]
fn n11_review_requesting_run_gpu_floor() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| snapshot.free_gpu_bytes = 3 * GIB);
    let mut bounds = limits();
    bounds.gpu_reserve_bytes = (3 * GIB).try_into().unwrap();
    let result = resources
        .reserve(
            &[bounds],
            Usage {
                gpu_bytes: 1,
                ..Usage::default()
            },
        )
        .map(|_| ());
    println!("requesting run GPU floor: result={result:?}");
    assert_eq!(result, Err(Pending::Gpu));
}

#[test]
fn n11_review_combined_disk_reserve() {
    let (controls, resources) = resources();
    change(&controls, |snapshot| snapshot.free_disk_bytes = 32 * GIB);
    let held = reserve(
        &resources,
        Usage {
            staging_bytes: GIB,
            ..Usage::default()
        },
    );
    let result = resources
        .reserve(
            &[limits()],
            Usage {
                staging_bytes: GIB + 1,
                ..Usage::default()
            },
        )
        .map(|_| ());
    println!(
        "combined disk reserve: result={result:?}, usage={:?}, pending={:?}",
        resources.usage().unwrap(),
        held.pending().unwrap()
    );
    assert_eq!(result, Err(Pending::DiskReserve));
}
