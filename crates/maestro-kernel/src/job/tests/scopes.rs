//! Scopes: a job is read only through a set that covers the scope it works
//! on, as every read of the kernel is, and one it does not cover reads as
//! unknown.

use super::support::{SCOPE, Scratch, TERM, at, collection, publish, running};
use crate::{
    job::{Error, NewJob},
    scope::{Scope, ScopeSet},
};
use serde_json::json;

/// The set of the scopes `paths` and every scope below them, as the grants
/// of a principal give it.
fn granted(paths: &[&str]) -> ScopeSet {
    ScopeSet::new(paths.iter().map(|path| path.parse().unwrap()).collect())
}

#[test]
fn a_job_and_its_steps_are_read_only_through_a_set_that_covers_its_scope() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, mut lease) = running(&database, "demo");
    let step = database
        .progress(&mut lease, at(1), TERM, &json!({"step": 1}))
        .unwrap();
    let covering = [
        granted(&[SCOPE]),
        granted(&["workspace/default"]),
        granted(&["workspace/default/collection/other", SCOPE]),
    ];
    for scopes in covering {
        let found = database.job(&scopes, job.id).unwrap().unwrap();
        assert_eq!(found.id, job.id);
        let last = database.last_progress(&scopes, job.id).unwrap();
        assert_eq!(last, Some(step.clone()));
    }
    let below = format!("{SCOPE}/source/docs");
    let blind = [
        granted(&[]),
        granted(&["workspace/default/collection/other"]),
        granted(&["workspace/default/collection/dem"]),
        granted(&["workspace/other"]),
        granted(&[below.as_str()]),
    ];
    for scopes in blind {
        assert_eq!(database.job(&scopes, job.id).unwrap(), None, "{scopes:?}");
        let refused = database.last_progress(&scopes, job.id).unwrap_err();
        assert!(
            matches!(refused, Error::UnknownJob(id) if id == job.id),
            "{scopes:?}: {refused:?}"
        );
    }
}

#[test]
fn the_same_work_in_another_scope_is_another_job_read_through_its_own() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = collection("demo");
    let job = database.submit_job(&publish(&inputs), at(0)).unwrap();
    let other: Scope = "workspace/default/collection/other".parse().unwrap();
    let elsewhere = NewJob {
        scope: &other,
        ..publish(&inputs)
    };
    let moved = database.submit_job(&elsewhere, at(1)).unwrap();
    assert_ne!(moved.id, job.id);
    assert_eq!(moved.scope, other);
    let theirs = granted(&["workspace/default/collection/other"]);
    assert_eq!(database.job(&theirs, moved.id).unwrap(), Some(moved));
    assert_eq!(database.job(&theirs, job.id).unwrap(), None);
    assert_eq!(database.job(&granted(&[SCOPE]), job.id).unwrap(), Some(job));
}
