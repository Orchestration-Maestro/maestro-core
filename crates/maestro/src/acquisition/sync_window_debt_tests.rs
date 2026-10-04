//! Exact staging ceilings and frozen pending-window boundaries.
use super::{sync_drain::Drain, sync_window::charge};
use crate::acquisition::{
    flow_fixture::{Fixture, clean},
    mutation_support::with_work,
};
use maestro_acquisition::{
    lifecycle::full::Mode,
    transport::{budget::Usage, stream::Accounting},
};
use maestro_kernel::acquisition::{Status, Window};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn debt_window_staging_charge_accepts_exact_ceiling_and_refuses_excess() {
    let fixture = Fixture::new(clean);
    with_work(&fixture, SystemTime::now(), |_, resources, run| {
        let limits = run.bounds[0].clone();
        let ceiling = limits.staging_bytes.get();
        let reservation = run.reserve(resources, Usage::default()).unwrap();
        let mut state = Drain {
            watermark: None,
            due_window: Window {
                start: 0,
                end: 1,
                overlap: 0,
                skew: 0,
            },
            window: Window {
                start: 0,
                end: 1,
                overlap: 0,
                skew: 0,
            },
            depths: BTreeMap::new(),
            captured_children: BTreeSet::new(),
            exhausted: false,
            reservation: Some(reservation),
            accounting: Accounting::new(limits),
            usage: Usage::default(),
            carried_staging: 0,
            aggregate_bounds: &run.bounds,
            seen: BTreeSet::new(),
            attempted: 0,
            inventory: BTreeMap::new(),
            keys: BTreeMap::new(),
            partitions: vec![],
        };
        assert!(charge(&mut state, ceiling).unwrap());
        assert_eq!(resources.usage().unwrap().staging_bytes, ceiling);
        assert!(!charge(&mut state, 1).unwrap());
        assert_eq!(state.usage.staging_bytes, ceiling);
    });
    fixture.finish();
}

#[test]
fn debt_window_continuation_accepts_pending_target_equal_to_now() {
    let fixture = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["limits"]["pages"] = json!(2);
    });
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert_eq!(
        fixture.sync_window(Mode::Incremental, now).status,
        Status::Partial
    );
    let continued = fixture.sync_window(Mode::Incremental, now);
    assert_eq!(continued.status, Status::Complete, "{continued:?}");
    fixture.finish();
}

#[test]
fn debt_window_recovery_skips_crashed_target_at_accepted_watermark() {
    use super::sync_window::recover_target;
    let fixture = Fixture::new(clean);
    let old = UNIX_EPOCH + Duration::from_secs(2_000_000);
    with_work(&fixture, old, |_, _, _| {});
    with_work(&fixture, old + Duration::from_secs(5), |work, _, _| {
        assert!(recover_target(work, Some(2_000_000_000)).unwrap().is_none());
    });
    fixture.finish();
}
