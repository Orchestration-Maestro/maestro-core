//! Production publication, factory, lease fences and immutable handle ownership.
use super::public_fixture::{Fixture, now, settings};
use crate::graph::projection::{
    BuildVerification, EdgeFamily, ProjectionCancellation, ProjectionEngine, ProjectionError,
    ProjectionFactory, ProjectionHandle, TypedEdgeProjection, content,
};
use lbug::Connection;
use maestro_filesystem::{ControlFile, FileLock, LockMode, OwnedRoot, SystemFileLock};
use maestro_kernel::artifact::Digest;
#[cfg(not(windows))]
use maestro_kernel::job::JobState;
#[cfg(not(windows))]
use std::cell::Cell;
use std::{fs, io, sync::Mutex};

fn expected(fixture: &Fixture) -> BuildVerification {
    BuildVerification::expected(&fixture.edges, &[]).unwrap()
}

#[cfg(not(windows))]
pub(super) fn publish(fixture: &Fixture) {
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
    assert_eq!(producer.settings(), &settings());
    producer.write_batch(&fixture.edges, &[]).unwrap();
    assert_eq!(producer.verify().unwrap(), expected(fixture));
    let published = producer.publish(&expected(fixture)).unwrap();
    assert_eq!(published.settings, settings());
    assert_eq!(
        fixture
            .authority
            .database
            .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
            .unwrap(),
        Some(published.receipt)
    );
}

// Windows native writes refuse; fixture creation is the existing legacy read-only pattern.
#[cfg(windows)]
pub(super) fn publish(fixture: &Fixture) {
    use super::{
        config::native, schema::tests::install_reader_fixture,
        transaction::tests::populate_reader_fixture,
    };
    use crate::graph::projection::writer::receipt_from_verification;
    use lbug::{Connection, Database};
    let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
    {
        #[expect(
            clippy::disallowed_methods,
            reason = "Windows immutable reader fixture only; production writes refuse"
        )]
        let database = Database::new(fixture.native.path.join(&name), native(&settings())).unwrap();
        let connection = Connection::new(&database).unwrap();
        install_reader_fixture(&connection, &fixture.build.scope);
        populate_reader_fixture(&connection, &fixture.build.scope, &fixture.edges, &[]);
        connection.query("CHECKPOINT").unwrap();
    }
    super::open::tests::private_windows_fixture(&fixture.native.path);
    let receipt = receipt_from_verification(
        &fixture.build.scope,
        fixture.build.claim_set_id.clone(),
        name,
        &expected(fixture),
    )
    .unwrap();
    fixture
        .authority
        .database
        .record_projection_ready(
            &fixture.authority.scopes,
            &receipt,
            &fixture.build.lease,
            now(0),
        )
        .unwrap();
}

#[test]
fn lifecycle_readiness_physical_binding_registry_and_guard_lifetimes() {
    let fixture = Fixture::new();
    let factory = fixture.factory();
    assert!(
        factory
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone()
            )
            .is_err()
    );
    publish(&fixture);
    let first = factory
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    let second = fixture
        .factory()
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    assert_eq!(first.settings(), &settings());
    let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
    assert_eq!(super::registry::owner_count(&fixture.native.path, &name), 3);
    let held = OwnedRoot::open(&fixture.native.path, false).unwrap();
    let cleanup = held.open_control(ControlFile::Access).unwrap();
    assert!(
        cleanup
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    assert_scoped_reads(&fixture, &[&first, &second]);
    let changed = EngineSettings::new(
        32 * 1024 * 1024,
        64 * 1024 * 1024,
        1,
        Digest::of(b"changed lock"),
    )
    .unwrap();
    let incompatible = ProjectionFactory::new(
        &fixture.native.path,
        ProjectionEngine::Ladybug,
        changed,
        &SystemFileLock,
    );
    assert!(
        incompatible
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone()
            )
            .is_err()
    );
    drop(first);
    assert!(
        cleanup
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    drop(second);
    assert_eq!(super::registry::owner_count(&fixture.native.path, &name), 0);
    cleanup
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    drop(cleanup);
    drop(
        factory
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .unwrap(),
    );
}

use crate::graph::projection::EngineSettings;

struct Unsupported;
impl FileLock for Unsupported {
    fn acquire(&self, _file: &fs::File, _mode: LockMode, _wait: bool) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[test]
fn lifecycle_unsupported_reader_and_writer_locks_fail_closed_without_native_io() {
    let fixture = Fixture::new();
    let factory = ProjectionFactory::new(
        &fixture.native.path,
        ProjectionEngine::Ladybug,
        settings(),
        &Unsupported,
    );
    let clock = || now(0);
    let refused = factory.producer(
        &fixture.authority.database,
        &fixture.authority.scopes,
        fixture.build.clone(),
        &clock,
    );
    assert!(matches!(refused, Err(ProjectionError::Backend(message))
        if message.contains("unsupported")));
    let refused = factory.reader(
        &fixture.authority.database,
        &fixture.authority.scopes,
        fixture.build.scope.clone(),
    );
    assert!(matches!(refused, Err(ProjectionError::Backend(message))
        if message.contains("unsupported")));
    assert_eq!(fs::read_dir(&fixture.native.path).unwrap().count(), 2);
    publish(&fixture);
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
fn lifecycle_serialized_writer_cancellation_preserves_orphan_and_releases_access() {
    let fixture = Fixture::new();
    let clock = || now(0);
    let factory = fixture.factory();
    let mut producer = factory
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.write_batch(&fixture.edges, &[]).unwrap();
    assert!(
        factory
            .producer(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock
            )
            .is_err()
    );
    let root = OwnedRoot::open(&fixture.native.path, false).unwrap();
    let cleanup = root.open_control(ControlFile::Access).unwrap();
    assert!(
        cleanup
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    producer.cancel().unwrap();
    cleanup
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    let state = fixture
        .authority
        .database
        .job(&fixture.authority.scopes, fixture.build.lease.job)
        .unwrap()
        .unwrap()
        .state;
    assert_eq!(state, JobState::Cancelled);
    assert_eq!(fs::read_dir(&fixture.native.path).unwrap().count(), 3);
    let orphan = fs::read_dir(&fixture.native.path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.is_dir())
        .unwrap();
    assert!(
        orphan
            .join(content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap())
            .is_file()
    );
}

#[cfg(not(windows))]
#[test]
fn lifecycle_existing_final_and_expiry_during_install_never_create_readiness() {
    for existing_final in [true, false] {
        let fixture = Fixture::new();
        let calls = Cell::new(0);
        let expire = Cell::new(false);
        let clock = || {
            let call = calls.get();
            calls.set(call + 1);
            if expire.get() && call >= 2 {
                now(60)
            } else {
                now(0)
            }
        };
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
        let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
        let final_path = fixture.native.path.join(&name);
        if existing_final {
            fs::write(&final_path, b"preserved old final").unwrap();
        } else {
            calls.set(0);
            expire.set(true);
        }
        assert!(
            matches!(producer.publish(&expected(&fixture)), Err(ProjectionError::Backend(message))
                if message.contains("writer closed") && message.contains("explicit recovery"))
        );
        if existing_final {
            assert_eq!(fs::read(&final_path).unwrap(), b"preserved old final");
        } else {
            assert!(final_path.is_file());
        }
        assert!(
            fixture
                .authority
                .database
                .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
                .unwrap()
                .is_none()
        );
        let root = OwnedRoot::open(&fixture.native.path, false).unwrap();
        root.open_control(ControlFile::Writer)
            .unwrap()
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .unwrap();
    }
    publish(&Fixture::new());
}

#[test]
fn lifecycle_cancellable_reader_refuses_rows_but_does_not_cancel_another_reader() {
    let fixture = Fixture::new();
    publish(&fixture);
    let token = ProjectionCancellation::new();
    let cancelled = fixture
        .factory()
        .reader_cancellable(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
            token.clone(),
        )
        .unwrap();
    token.cancel();
    assert!(
        cancelled
            .neighbors(
                &fixture.authority.scopes,
                &fixture.build.scope,
                EdgeFamily::KnowledgeClaim,
                &fixture.edges[0].source
            )
            .is_err()
    );
    let reader = fixture
        .factory()
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    assert_eq!(
        reader
            .neighbors(
                &fixture.authority.scopes,
                &fixture.build.scope,
                EdgeFamily::KnowledgeClaim,
                &fixture.edges[0].source
            )
            .unwrap(),
        fixture.edges
    );
}

#[test]
fn lifecycle_native_read_query_is_interrupted_from_a_synchronized_thread() {
    use super::tests::Fixture as NativeFixture;
    use lbug::Connection;
    use std::{
        sync::{Mutex, mpsc},
        thread,
    };
    let fixture = NativeFixture::new();
    {
        let writer = fixture.writer();
        Connection::new(&writer)
            .unwrap()
            .query("CHECKPOINT")
            .unwrap();
    }
    let reader = fixture.reader();
    let connection = Connection::new(&reader).unwrap();
    let token = ProjectionCancellation::new();
    let native_error = Mutex::new(None);
    let (entered, wait) = mpsc::sync_channel(0);
    thread::scope(|scope| {
        let read = scope.spawn(|| {
            super::cancellation::run(&connection, &token, || {
                entered.send(()).unwrap();
                long_query(&connection, &native_error)
            })
        });
        wait.recv().unwrap();
        token.cancel();
        assert!(read.join().unwrap().is_err());
    });
    let error = native_error
        .into_inner()
        .unwrap()
        .expect("a real native query must have been interrupted");
    assert!(error.contains("Interrupted"), "{error}");
    assert!(
        connection.query("RETURN 1").is_ok(),
        "cancellation is connection-local and reset at the next query"
    );
}

fn assert_scoped_reads(fixture: &Fixture, readers: &[&ProjectionHandle]) {
    for reader in readers {
        assert_eq!(
            reader
                .neighbors(
                    &fixture.authority.scopes,
                    &fixture.build.scope,
                    EdgeFamily::KnowledgeClaim,
                    &fixture.edges[0].source
                )
                .unwrap(),
            fixture.edges
        );
        assert_eq!(
            reader
                .neighbors(
                    &fixture.authority.scopes,
                    &fixture.build.scope,
                    EdgeFamily::CatalogDependency,
                    &fixture.edges[0].source
                )
                .unwrap(),
            vec![]
        );
        let denied = fixture.authority.database.visible("unknown").unwrap();
        assert_eq!(
            reader.neighbors(
                &denied,
                &fixture.build.scope,
                EdgeFamily::KnowledgeClaim,
                &fixture.edges[0].source
            ),
            Err(ProjectionError::Unauthorized)
        );
    }
}

fn long_query(
    connection: &Connection<'_>,
    native_error: &Mutex<Option<String>>,
) -> Result<(), String> {
    let result = connection
        .query("UNWIND range(1, 100000) AS a UNWIND range(1, 100000) AS b RETURN sum(a * b)");
    match result {
        Ok(_) => Ok(()),
        Err(error) => {
            let error = error.to_string();
            *native_error.lock().unwrap() = Some(error.clone());
            Err(error)
        }
    }
}

#[cfg(not(windows))]
#[test]
fn lifecycle_ready_generation_cannot_start_a_second_session_or_create_an_orphan() {
    let fixture = Fixture::new();
    publish(&fixture);
    let before = fs::read_dir(&fixture.native.path).unwrap().count();
    let clock = || now(0);
    assert!(
        fixture
            .factory()
            .producer(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock
            )
            .is_err()
    );
    assert_eq!(fs::read_dir(&fixture.native.path).unwrap().count(), before);
}
