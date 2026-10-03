//! Active native readers refuse independently changed settings and frozen lock identities.
use super::{
    public_fixture::{Fixture, settings},
    public_tests::publish,
};
use crate::graph::projection::{
    EngineSettings, ProjectionEngine, ProjectionError, ProjectionFactory,
};
use maestro_filesystem::SystemFileLock;
use maestro_kernel::artifact::Digest;
use std::{fs, time::Instant};

#[test]
fn lifecycle_active_readers_refuse_changed_settings_or_changed_complete_lock() {
    let fixture = Fixture::new();
    publish(&fixture);
    let factory = fixture.factory();
    let started = Instant::now();
    let active = factory
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    let opened = started.elapsed();
    for changed in [
        EngineSettings::new(
            32 * 1024 * 1024,
            64 * 1024 * 1024,
            1,
            settings().frozen_lock().clone(),
        )
        .unwrap(),
        EngineSettings::new(
            16 * 1024 * 1024,
            64 * 1024 * 1024,
            1,
            Digest::of(b"changed complete lock only"),
        )
        .unwrap(),
    ] {
        let incompatible = ProjectionFactory::new(
            &fixture.native.path,
            ProjectionEngine::Ladybug,
            changed,
            &SystemFileLock,
        );
        assert_eq!(
            incompatible
                .reader(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.scope.clone()
                )
                .unwrap_err(),
            ProjectionError::NotReady
        );
    }
    let matching = factory
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    assert_eq!(matching.settings(), active.settings());
    drop(matching);
    drop(active);
    let started = Instant::now();
    let reopened = factory
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    let elapsed = started.elapsed();
    let receipt = fixture
        .authority
        .database
        .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
        .unwrap()
        .unwrap();
    let bytes = fs::metadata(fixture.native.path.join(receipt.file_name))
        .unwrap()
        .len();
    println!(
        "G26_NATIVE_OPEN_US={} REOPEN_US={} GRAPH_DISK_BYTES={bytes}",
        opened.as_micros(),
        elapsed.as_micros()
    );
    assert_eq!(reopened.settings(), &settings());
}
