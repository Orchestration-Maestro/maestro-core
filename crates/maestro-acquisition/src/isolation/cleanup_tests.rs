//! Deadline-bounded whole-tree collection without host kernel effects.
use super::{port::Refusal, supervision::wait_empty};
use crate::isolation::test_support::FixedClock;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

#[test]
fn n17_cleanup_never_empty_refuses_at_deadline_without_spinning() {
    let now = Instant::now();
    let clock = FixedClock(Mutex::new(now));
    let mut observations = 0;
    let result = wait_empty(
        || {
            observations += 1;
            assert!(observations <= 2, "cleanup ignored its deadline");
            *clock.0.lock().unwrap() += Duration::from_millis(1);
            Ok(false)
        },
        now + Duration::from_millis(2),
        &clock,
    );
    assert_eq!(result, Err(Refusal::Cleanup));
    assert_eq!(observations, 2);
}
#[test]
fn n17_cleanup_empty_returns_immediately_even_at_expiry() {
    let now = Instant::now();
    let clock = FixedClock(Mutex::new(now));
    let mut observations = 0;
    assert_eq!(
        wait_empty(
            || {
                observations += 1;
                assert_eq!(observations, 1);
                Ok(true)
            },
            now,
            &clock
        ),
        Ok(())
    );
    assert_eq!(observations, 1);
    assert_eq!(
        wait_empty(|| Err(Refusal::Cleanup), now, &clock),
        Err(Refusal::Cleanup)
    );
}
