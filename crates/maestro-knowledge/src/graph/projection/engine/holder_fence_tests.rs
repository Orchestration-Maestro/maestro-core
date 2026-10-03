//! Holder credentials refuse before reservation, without changing authoritative state.
use super::public_fixture::{Fixture, now};
use crate::graph::projection::ProjectionError;
#[cfg(not(windows))]
use maestro_kernel::job::JobState;
use maestro_kernel::{job, journal::Filter};
use std::{cell::Cell, collections::BTreeSet, fs};

#[test]
fn lifecycle_holder_mismatch_refuses_before_reservation() {
    let fixture = Fixture::new();
    let database = &fixture.authority.database;
    let scopes = &fixture.authority.scopes;
    let build = &fixture.build;
    let snapshot = || {
        (
            database.job(scopes, build.lease.job).unwrap(),
            database
                .events(
                    scopes,
                    &Filter {
                        stream: &job::stream(build.lease.job),
                        after: 0,
                        r#type: None,
                    },
                )
                .unwrap(),
            database
                .projection_ready(scopes, build.scope.generation_id)
                .unwrap(),
            fs::read_dir(&fixture.native.path)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<BTreeSet<_>>(),
        )
    };
    let before = snapshot();
    let time = Cell::new(now(0));
    let clock = || time.get();
    let factory = fixture.factory();
    let mut wrong = build.clone();
    wrong.lease.holder = "wrong-holder".to_owned();
    let expected = job::Error::Lost {
        job: build.lease.job,
        holder: wrong.lease.holder.clone(),
        number: build.lease.number,
    }
    .to_string();
    let error = factory
        .producer(database, scopes, wrong, &clock)
        .unwrap_err();
    let ProjectionError::Backend(message) = error else {
        panic!("{error:?}")
    };
    assert_eq!(message, expected);
    assert_eq!(
        snapshot(),
        before,
        "refusal must not reserve or change kernel state"
    );
    #[cfg(not(windows))]
    {
        let producer = factory
            .producer(database, scopes, build.clone(), &clock)
            .unwrap();
        // The genuine holder may still cancel at expiry; cancellation adds no freshness check.
        time.set(now(60));
        producer.cancel().unwrap();
        assert_eq!(
            database
                .job(scopes, build.lease.job)
                .unwrap()
                .unwrap()
                .state,
            JobState::Cancelled
        );
    }
}
