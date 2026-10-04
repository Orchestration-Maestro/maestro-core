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

pub(super) fn fixture() -> Fixture {
    let mut fixture = Fixture::new();
    reserve(&mut fixture, None);
    fixture
}

pub(super) fn reserve(fixture: &mut Fixture, predecessor: Option<i64>) {
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
fn cutover_resume_rechecks_expiry_before_each_checkpoint_write() {
    for uncertain in [true, false] {
        let fixture = fixture();
        let rows = snapshot(&fixture);
        let clock = || now(0);
        let mut producer = fixture
            .factory()
            .producer(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock,
            )
            .unwrap();
        if uncertain {
            producer.write_batch(&rows.edges, &rows.facts).unwrap();
        }
        Journal::create(
            &staging(&fixture),
            Manifest::expected(&fixture.build, &rows, None).unwrap(),
        )
        .unwrap();
        drop(producer);
        let calls = Cell::new(0);
        // Admit and verify precede uncertain recording. Fresh batches also fence
        // write_batch and its verification before the independent record fence.
        let expiry_call = if uncertain { 3 } else { 5 };
        let clock = || {
            let n = calls.get() + 1;
            calls.set(n);
            now(if n >= expiry_call { 61 } else { 0 })
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
        assert!(resumed.load(&rows).is_err(), "uncertain={uncertain}");
        assert!(!staging(&fixture).join("loader/0000000001.json").exists());
    }
}

#[test]
fn cutover_resume_refuses_changed_head_and_foreign_or_expired_lease_before_native_open() {
    use crate::graph::projection::{EngineSettings, ProjectionEngine, ProjectionFactory};
    use maestro_filesystem::SystemFileLock;
    use maestro_kernel::artifact::Digest;
    let mut fixture = fixture();
    prepared(&fixture).unwrap();
    let loser = fixture.build.clone();
    let clock = || now(0);
    let mut foreign = loser.clone();
    foreign.lease.holder = "foreign".into();
    for (build, time) in [(foreign, now(0)), (loser.clone(), now(61))] {
        super::open::tests::OPEN_CALLS.set(0);
        let clock = || time;
        assert!(
            fixture
                .factory()
                .resume(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    build,
                    &clock
                )
                .is_err()
        );
        assert_eq!(super::open::tests::OPEN_CALLS.get(), 0);
    }
    let admitted = EngineSettings::new(
        16 * 1024 * 1024,
        64 * 1024 * 1024,
        1,
        Digest::of(b"competing frozen lock"),
    )
    .unwrap();
    fixture.build.settings_identity = admitted.identity();
    fixture.build.frozen_lock = admitted.frozen_lock().clone();
    reserve(&mut fixture, None);
    let factory = ProjectionFactory::new(
        &fixture.native.path,
        ProjectionEngine::Ladybug,
        admitted,
        &SystemFileLock,
    );
    let mut winner = factory
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    let rows = snapshot(&fixture);
    winner.load(&rows).unwrap();
    winner
        .publish(&BuildVerification::expected(&rows.edges, &rows.facts).unwrap())
        .unwrap();
    super::open::tests::OPEN_CALLS.set(0);
    assert!(
        fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                loser.clone(),
                &clock
            )
            .is_err()
    );
    assert_eq!(super::open::tests::OPEN_CALLS.get(), 0);
    assert!(
        fixture
            .native
            .path
            .join(format!(".build-{}", loser.lease.job))
            .join("loader/manifest.json")
            .is_file()
    );
}
