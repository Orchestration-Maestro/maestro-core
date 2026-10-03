//! Every public open refuses a half-set-up root, without creating a replacement guard.
use super::{public_fixture::Fixture, public_tests::publish};
#[cfg(not(windows))]
use crate::graph::projection::CatalogRelationVocabulary;
use crate::graph::projection::ProjectionError;
use maestro_canonicalization::{ControlFile, OwnedRoot};
use std::fs;

#[test]
fn lifecycle_reader_requires_both_permanent_guards_and_valid_neighbor_reopens() {
    let fixture = Fixture::new();
    publish(&fixture);
    let root = OwnedRoot::open(&fixture.native.path, false).unwrap();
    for missing in [ControlFile::Access, ControlFile::Writer] {
        drop(
            fixture
                .factory()
                .reader(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.scope.clone(),
                )
                .unwrap(),
        );
        fs::remove_file(fixture.native.path.join(missing.file_name())).unwrap();
        let error = fixture
            .factory()
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .unwrap_err();
        assert!(matches!(error, ProjectionError::Backend(message)
                if message.contains(missing.file_name())
                && message.contains("maestro setup --yes")));
        assert!(!fixture.native.path.join(missing.file_name()).exists());
        assert!(root.ensure_control(missing).unwrap());
    }
    drop(
        fixture
            .factory()
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .unwrap(),
    );
}

#[cfg(not(windows))]
#[test]
fn lifecycle_authoritative_expectation_matches_both_native_families_and_rejects_tampered_row() {
    use super::public_fixture::now;
    use crate::graph::projection::{
        BuildVerification, CatalogRelationVocabulary, EdgeFamily, content,
    };
    use maestro_kernel::artifact::Digest;
    struct Vocabulary;
    impl CatalogRelationVocabulary for Vocabulary {
        fn accepts(&self, relation: &str) -> bool {
            relation == "depends_on"
        }
    }
    let fixture = Fixture::new();
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
    let mut edges = fixture.edges.clone();
    let mut catalog = edges[0].clone();
    catalog.id = Digest::of(b"catalog dependency");
    catalog.family = EdgeFamily::CatalogDependency;
    catalog.relation = "depends_on".into();
    edges.push(catalog);
    let expected =
        BuildVerification::expected_with_catalog_vocabulary(&edges, &[], &Vocabulary).unwrap();
    producer
        .write_batch_with_catalog_vocabulary(&edges, &[], &Vocabulary)
        .unwrap();
    assert_eq!(producer.verify().unwrap(), expected);
    producer.publish(&expected).unwrap();

    let fixture = Fixture::new();
    let expected = BuildVerification::expected(&fixture.edges, &[]).unwrap();
    let mut tampered = fixture.edges[0].clone();
    tampered.target = Digest::of(b"wrong native target");
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.write_batch(&[tampered], &[]).unwrap();
    assert_eq!(producer.publish(&expected), Err(ProjectionError::NotReady));
    assert!(
        fixture
            .authority
            .database
            .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
            .unwrap()
            .is_none()
    );
    let final_name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
    assert!(!fixture.native.path.join(final_name).exists());
}

#[cfg(not(windows))]
#[test]
fn lifecycle_expired_current_lease_cancels_but_takeover_cannot_be_cancelled_by_old_token() {
    struct Vocabulary;
    impl CatalogRelationVocabulary for Vocabulary {
        fn accepts(&self, _relation: &str) -> bool {
            false
        }
    }
    use super::public_fixture::now;
    use maestro_canonicalization::{LockMode, SystemFileLock};
    use maestro_kernel::job::JobState;
    use std::{cell::Cell, time::Duration};
    for takeover in [false, true] {
        let fixture = Fixture::new();
        let time = Cell::new(now(0));
        let clock = || time.get();
        let mut producer = fixture
            .factory()
            .producer(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock,
            )
            .unwrap();
        producer.write_batch(&fixture.edges, &[]).unwrap();
        time.set(now(60));
        let refused = producer.write_batch(&fixture.edges, &[]).unwrap_err();
        assert_eq!(
            producer.write_batch_with_catalog_vocabulary(&[], &[], &Vocabulary),
            Err(refused)
        );
        let successor = takeover.then(|| {
            fixture
                .authority
                .database
                .take_job(
                    fixture.build.lease.job,
                    "successor",
                    now(60),
                    Duration::from_secs(60),
                )
                .unwrap()
        });
        let result = producer.cancel();
        assert_eq!(result.is_err(), takeover);
        let job = fixture
            .authority
            .database
            .job(&fixture.authority.scopes, fixture.build.lease.job)
            .unwrap()
            .unwrap();
        if let Some(successor) = successor {
            assert_eq!(job.state, JobState::Running);
            assert_eq!(job.lease.unwrap().number, successor.number);
        } else {
            assert_eq!(job.state, JobState::Cancelled);
        }
        let orphan = fs::read_dir(&fixture.native.path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.is_dir())
            .unwrap();
        assert!(fs::read_dir(orphan).unwrap().next().is_some());
        let root = OwnedRoot::open(&fixture.native.path, false).unwrap();
        for control in [ControlFile::Access, ControlFile::Writer] {
            root.open_control(control)
                .unwrap()
                .lock_with(&SystemFileLock, LockMode::Exclusive, false)
                .unwrap();
        }
    }
}

// Unix private-mode failure is exercised here; Windows has its separately qualified ACL boundary.
#[cfg(not(windows))]
#[test]
fn lifecycle_unsafe_control_is_preserved_and_move_aside_repair_opens_valid_neighbor() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    publish(&fixture);
    let path = fixture.native.path.join(ControlFile::Writer.file_name());
    let payload = b"preserved unsafe control bytes";
    fs::write(&path, payload).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let error = fixture
        .factory()
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap_err();
    assert!(matches!(error, ProjectionError::Backend(message)
        if message.contains(&path.display().to_string())
        && message.contains("move it aside")
        && message.contains("won't replace")
        && message.contains("maestro setup --yes")));
    assert_eq!(fs::read(&path).unwrap(), payload);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o644
    );
    let preserved = fixture.native.path.join("preserved-unsafe.guard");
    fs::rename(&path, &preserved).unwrap();
    let root = OwnedRoot::open(&fixture.native.path, false).unwrap();
    assert!(root.ensure_control(ControlFile::Writer).unwrap());
    drop(
        fixture
            .factory()
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .unwrap(),
    );
    assert_eq!(fs::read(preserved).unwrap(), payload);
}

#[test]
fn guard_cancel_factory_pre_cancelled_open_makes_zero_native_constructions() {
    use super::open::tests::OPEN_CALLS;
    use crate::graph::projection::ProjectionCancellation;
    let fixture = Fixture::new();
    publish(&fixture);
    let token = ProjectionCancellation::new();
    token.cancel();
    OPEN_CALLS.set(0);
    let error = fixture
        .factory()
        .reader_cancellable(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
            token,
        )
        .unwrap_err();
    assert_eq!(
        error,
        ProjectionError::Backend("projection read cancelled".into())
    );
    assert_eq!(OPEN_CALLS.get(), 0);
}
