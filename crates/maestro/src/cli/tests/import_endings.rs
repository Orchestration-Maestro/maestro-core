//! How an import run as a job ends it: succeeded with its report, or failed
//! saying why; and how each step it shows is journaled, or stops it once
//! the lease is lost.

use super::support::{Scratch, everything, leased, lost};
use crate::cli::{
    import::{ending, step},
    lease::{Holder, TIMING},
    output::Output,
};
use maestro_kernel::job::{self, JobState};
use maestro_knowledge::import::{self, Report};
use serde_json::json;
use std::{
    ops::ControlFlow,
    time::{Duration, SystemTime},
};
use ulid::Ulid;

/// The report of an import that imported two documents and found one
/// unchanged.
fn report() -> Report {
    Report {
        collection: "synthetic".to_owned(),
        imported: 2,
        unchanged: 1,
        held: 0,
        refused: 0,
        refusals: Vec::new(),
    }
}

#[test]
fn an_import_that_ended_makes_its_job_succeed_with_its_report() {
    assert_eq!(
        ending(Ok(report()), None),
        (
            JobState::Succeeded,
            json!({
                "collection": "synthetic",
                "imported": 2,
                "unchanged": 1,
                "held": 0,
                "refused": 0,
                "refusals": [],
            })
        )
    );
}

#[test]
fn an_import_that_stopped_makes_its_job_fail_saying_why() {
    let hidden = import::Error::NotVisible("workspace/default/collection/synthetic".to_owned());
    let message = hidden.to_string();
    assert_eq!(
        ending(Err(hidden), None),
        (JobState::Failed, json!({ "error": message }))
    );
    let lost = job::Error::Lost {
        job: Ulid::nil(),
        holder: "cli".to_owned(),
        number: 1,
    };
    let message = lost.to_string();
    assert_eq!(
        ending(Ok(report()), Some(lost)),
        (JobState::Failed, json!({ "error": message })),
        "an import whose steps could not all be journaled fails"
    );
}

#[test]
fn a_step_journaled_under_the_lease_lets_the_import_go_on() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let taken = leased(&database, "cli", SystemTime::now(), Duration::from_secs(60));
    Holder::run(&database, taken.clone(), TIMING, |holder| {
        let mut stopped = None;
        let next = step(holder, Output::new(true), &report(), &mut stopped);
        assert_eq!(next, ControlFlow::Continue(()));
        assert!(stopped.is_none(), "{stopped:?}");
        (JobState::Succeeded, json!({}))
    })
    .unwrap();
    let progressed = database.last_progress(&scopes, taken.job).unwrap().unwrap();
    assert_eq!(
        progressed.data,
        json!({ "imported": 2, "unchanged": 1, "held": 0, "refused": 0 })
    );
}

#[test]
fn a_step_under_a_lease_taken_over_stops_the_import_keeping_why() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let mut stopped = None;
    let ended = Holder::run(&database, lost(&database, "cli"), TIMING, |holder| {
        let next = step(holder, Output::new(true), &report(), &mut stopped);
        assert_eq!(next, ControlFlow::Break(()));
        (JobState::Failed, json!({}))
    });
    assert!(matches!(ended, Err(job::Error::Lost { .. })), "{ended:?}");
    assert!(
        matches!(stopped, Some(job::Error::Lost { .. })),
        "{stopped:?}"
    );
}
