//! Reserved loader /2 recovery; legacy runtime fixtures remain step 4 work.
use super::{
    loader_tests::{snapshot, staging},
    public_fixture::{Fixture, now},
};
use crate::graph::projection::{
    BuildVerification, ProjectionError,
    checkpoint::{Journal, Manifest},
    tests::reserved,
};
use std::{cell::Cell, fs};

fn fixture() -> Fixture {
    let mut fixture = Fixture::new();
    reserved::migrate(fixture.authority.directory());
    reserve(&mut fixture, None);
    fixture
}

fn reserve(fixture: &mut Fixture, predecessor: Option<i64>) {
    reserved::reserve(
        (&fixture.authority.database, &fixture.authority.scopes),
        &mut fixture.build,
        predecessor,
        now(0),
    );
}

fn prepared(fixture: &Fixture) -> Result<(), ProjectionError> {
    let clock = || now(0);
    let mut producer = fixture.factory().producer(
        &fixture.authority.database,
        &fixture.authority.scopes,
        fixture.build.clone(),
        &clock,
    )?;
    producer.load(&snapshot(fixture))
}

#[test]
fn cutover_resume_manifest_binds_every_reserved_field_before_native_open() {
    let fixture = fixture();
    prepared(&fixture).unwrap();
    let path = staging(&fixture).join("loader/manifest.json");
    let bytes = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for (field, changed) in [
        ("schema", serde_json::json!("graph-loader/1")),
        ("build_id", serde_json::json!(fixture.build.build_id + 1)),
        (
            "expected_active_build_id",
            serde_json::json!(fixture.build.build_id),
        ),
        ("native_schema", serde_json::json!("maestro-typed-edges/2")),
    ] {
        let mut changed_value = value.clone();
        changed_value[field] = changed;
        let changed_bytes = serde_json::to_vec(&changed_value).unwrap();
        fs::write(&path, &changed_bytes).unwrap();
        let clock = || now(0);
        assert!(
            matches!(
                fixture.factory().resume(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.clone(),
                    &clock
                ),
                Err(ProjectionError::Backend(_))
            ),
            "{field}"
        );
        assert_eq!(fs::read(&path).unwrap(), changed_bytes, "{field}");
    }
    fs::write(&path, bytes).unwrap();
    let clock = || now(0);
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    resumed.load(&snapshot(&fixture)).unwrap();
    resumed
        .publish(
            &BuildVerification::expected(&snapshot(&fixture).edges, &snapshot(&fixture).facts)
                .unwrap(),
        )
        .unwrap();
}

#[test]
fn cutover_resume_rechecks_expiry_before_uncertain_checkpoint_write() {
    let fixture = fixture();
    let rows = snapshot(&fixture);
    let clock = || now(0);
    // Leave a complete native batch but no checkpoint, as after a process death.
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.write_batch(&rows.edges, &rows.facts).unwrap();
    Journal::create(
        &staging(&fixture),
        Manifest::expected(&fixture.build, &rows, None).unwrap(),
    )
    .unwrap();
    drop(producer);
    let calls = Cell::new(0);
    // Resume admission uses call 1; verify call 2 succeeds; record call 3 expires.
    let clock = || {
        let n = calls.get() + 1;
        calls.set(n);
        now(if n >= 3 { 61 } else { 0 })
    };
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    assert!(resumed.load(&rows).is_err());
    assert!(!staging(&fixture).join("loader/0000000001.json").exists());
}
