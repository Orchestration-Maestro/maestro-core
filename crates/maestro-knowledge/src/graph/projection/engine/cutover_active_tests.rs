//! Real successive publications, held registry readers and current health selection.
use super::{
    cutover_resume_tests::{fixture, reserve},
    loader_tests::{snapshot, staging},
    probe::{Probe, ProbeReceipt},
    public_fixture::{Fixture, now, settings},
    registry,
};
use crate::graph::projection::{
    BuildVerification, EdgeFamily, ProjectionError, PublishedProjection, TypedEdgeProjection,
    content, health::PublishedFile as _, receipts::KernelProjectionReceipts,
};
use maestro_filesystem::SystemFileLock;
use maestro_kernel::scope::Right;
use std::{error::Error as StdError, fs};

fn publish(fixture: &Fixture) -> Result<PublishedProjection, ProjectionError> {
    let clock = || now(0);
    let rows = snapshot(fixture);
    let mut producer = fixture.factory().producer(
        &fixture.authority.database,
        &fixture.authority.scopes,
        fixture.build.clone(),
        &clock,
    )?;
    producer.load(&rows)?;
    producer.publish(&BuildVerification::expected(&rows.edges, &rows.facts)?)
}

#[test]
fn cutover_receiptless_final_is_preserved_and_refused_without_native_open() {
    let fixture = fixture();
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
    producer.load(&snapshot(&fixture)).unwrap();
    drop(producer);
    let name = content::build_basename(
        &fixture.build.scope,
        &fixture.build.claim_set_id,
        fixture.build.build_id,
    )
    .unwrap();
    let final_path = fixture.native.path.join(&name);
    fs::copy(staging(&fixture).join(&name), &final_path).unwrap();
    let bytes = fs::read(&final_path).unwrap();
    super::open::tests::OPEN_CALLS.set(0);
    assert!(
        fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock
            )
            .is_err()
    );
    assert_eq!(super::open::tests::OPEN_CALLS.get(), 0);
    assert_eq!(fs::read(&final_path).unwrap(), bytes);
    assert!(
        fixture
            .authority
            .database
            .projection_build_receipt(
                &fixture.authority.scopes,
                fixture.build.scope.generation_id,
                fixture.build.build_id
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn cutover_successive_builds_pin_held_readers_and_select_new_registry_and_health() {
    let mut fixture = fixture();
    let first = publish(&fixture).unwrap();
    fixture
        .authority
        .database
        .publish_generation(fixture.build.scope.generation_id)
        .unwrap();
    fixture
        .authority
        .database
        .grant(
            "local",
            &"workspace/default".parse().unwrap(),
            Right::Read,
            "cutover health",
        )
        .unwrap();
    let first_build = fixture.build.clone();
    let held = fixture
        .factory()
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    reserve(&mut fixture, Some(first_build.build_id));
    let second = publish(&fixture).unwrap();
    assert_ne!(
        first.receipt.identity.build_id,
        second.receipt.identity.build_id
    );
    assert_ne!(
        first.receipt.identity.file_name,
        second.receipt.identity.file_name
    );
    let current = fixture
        .factory()
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    assert_eq!(
        registry::owner_count(&fixture.native.path, &first.receipt.identity.file_name),
        2
    );
    assert_eq!(
        registry::owner_count(&fixture.native.path, &second.receipt.identity.file_name),
        2
    );
    for reader in [&held, &current] {
        let rows = snapshot(&fixture);
        let edge = &rows.edges[0];
        assert_eq!(
            reader
                .neighbors(
                    &fixture.authority.scopes,
                    &fixture.build.scope,
                    EdgeFamily::KnowledgeClaim,
                    &edge.source
                )
                .unwrap(),
            vec![edge.clone()]
        );
    }
    health_and_stamps(&fixture, &first, &second).unwrap();
    let clock = || now(0);
    for build in [first_build, fixture.build.clone()] {
        super::open::tests::OPEN_CALLS.set(0);
        let error = fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                build,
                &clock,
            )
            .unwrap_err();
        assert!(matches!(error, ProjectionError::Backend(_)));
        assert_eq!(super::open::tests::OPEN_CALLS.get(), 0);
    }
    drop(current);
    assert_eq!(
        registry::owner_count(&fixture.native.path, &second.receipt.identity.file_name),
        0
    );
    assert_eq!(
        registry::owner_count(&fixture.native.path, &first.receipt.identity.file_name),
        2
    );
    drop(held);
    assert_eq!(
        registry::owner_count(&fixture.native.path, &first.receipt.identity.file_name),
        0
    );
}

fn health_and_stamps(
    fixture: &Fixture,
    first: &PublishedProjection,
    second: &PublishedProjection,
) -> Result<(), Box<dyn StdError>> {
    let ProbeReceipt::Files(files) = Probe::receipt_with(
        &fixture.native.path,
        &SystemFileLock,
        &KernelProjectionReceipts::new(fixture.authority.directory()),
        Some(settings()),
    )
    .map_err(|error| format!("{error:?}"))?
    else {
        return Err("published health is absent".into());
    };
    assert_eq!(files.len(), 1);
    let file = files.first().ok_or("missing health file")?;
    assert_eq!(
        file.path(),
        fixture.native.path.join(&second.receipt.identity.file_name)
    );
    file.open_read_only()
        .map_err(|error| format!("{error:?}"))?
        .query_one()
        .map_err(|error| format!("{error:?}"))?;
    for published in [first, second] {
        let native = super::open::open(
            &fixture.native.root,
            &published.receipt.identity.file_name,
            super::tests::config().read_only(true),
        )?;
        let connection = lbug::Connection::new(&native)?;
        let mut result = connection.query("MATCH (p:Projection) RETURN p.build")?;
        assert_eq!(
            result.next().ok_or("missing native stamp")?,
            vec![lbug::Value::Int64(published.receipt.identity.build_id)]
        );
    }
    Ok(())
}
