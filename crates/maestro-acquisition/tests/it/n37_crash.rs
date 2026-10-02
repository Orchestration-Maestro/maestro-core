//! Kill a real capture writer between durable preparation and acknowledgement.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls,
    n09_support::Grants,
    n12_support::{Fixture, now},
    n37_support::{current, item, scope},
};
use maestro_acquisition::lifecycle::{
    full::same_capture,
    resume::{prepared, stop_owned},
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, DispatchRequest, Frontier, Handle, LeaseRequest,
    },
    retrieval::Clock,
    store::Database,
};
use std::{
    env, fs,
    io::{BufRead as _, BufReader, Write as _, stdout},
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

/// Independent fixture cleanup still reaps children when a hand mutant panics.
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        drop(self.0.kill());
        drop(self.0.wait());
    }
}

/// Host monotonic time is used only for safety deadlines, never asserted as a duration.
#[derive(Debug)]
struct HostClock;
impl Clock for HostClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Count authority reads to prove deadline refusal happens before sending a kill.
#[derive(Debug)]
struct CountedClock<'a> {
    inner: &'a dyn Clock,
    reads: AtomicUsize,
}
impl Clock for CountedClock<'_> {
    fn now(&self) -> Instant {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.now()
    }
}

#[test]
fn n37_capture_child_checkpoints_without_ack() {
    let Ok(root) = env::var("MAESTRO_N37_CAPTURE_ROOT") else {
        return;
    };
    let db = Database::open_in(Path::new(&root)).unwrap();
    let mut envelope: CaptureEnvelope =
        serde_json::from_slice(&fs::read(Path::new(&root).join("candidate.json")).unwrap())
            .unwrap();
    let at = now() + Duration::from_secs(1);
    let lease = LeaseRequest {
        holder: "crashing",
        now: at,
        term: Duration::from_secs(30),
    };
    let writer = db.lease_source("notes", &scope(), lease).unwrap();
    let row = Frontier::page(&db, &db.visible("reader").unwrap(), "notes", None, 1)
        .unwrap()
        .remove(0);
    let dispatch = db
        .lease(
            &writer,
            row.id,
            DispatchRequest {
                lease,
                max_attempts: 3,
            },
        )
        .unwrap();
    envelope.item = row.id.to_string().parse().unwrap();
    let capture = db
        .prepare_capture(
            &CaptureContext {
                writer,
                item: dispatch,
                now: at,
            },
            &envelope,
            b"body",
            u64::MAX,
        )
        .unwrap();
    println!("N37_PREPARED {}", capture.handle);
    stdout().flush().unwrap();
    loop {
        thread::park();
    }
}

#[test]
fn n37_killed_capture_writer_resumes_like_reference_without_duplicate_occurrence() {
    let mut fixture = Fixture::new();
    let (mut child, capture) = capture_writer(&fixture);
    let status = stop_owned(
        &mut child.0,
        Instant::now() + Duration::from_secs(30),
        &HostClock,
    )
    .unwrap();
    assert!(!status.success());
    assert!(
        child.0.try_wait().unwrap().is_some(),
        "owned capture process was not reaped"
    );
    assert!(
        item(&fixture).capture.is_none(),
        "preparation acknowledged completion"
    );
    fixture.context.now = now() + Duration::from_secs(32);
    let lease = LeaseRequest {
        holder: "resumed",
        now: fixture.context.now,
        term: Duration::from_secs(30),
    };
    fixture.context.writer = fixture.db.lease_source("notes", &scope(), lease).unwrap();
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            fixture.context.item.item,
            DispatchRequest {
                lease,
                max_attempts: 3,
            },
        )
        .unwrap();
    let controls = Controls::default();
    let grants = Grants::default();
    let access = current(&fixture, &fixture.policy, &controls, &grants);
    let (envelope, bytes) = prepared(
        &fixture.db,
        &access,
        (&item(&fixture), &fixture.context),
        capture,
        4,
    )
    .unwrap();
    assert_eq!(bytes.unwrap(), b"body");
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    let rows = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        100,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.first().unwrap().attempts, 3);
    assert_eq!(
        fixture
            .db
            .capture_for(&scope(), rows.first().unwrap())
            .unwrap(),
        Some(capture)
    );
    let reference = Fixture::new();
    let uninterrupted = reference.prepare().unwrap();
    reference
        .db
        .acknowledge_capture(&reference.context, uninterrupted)
        .unwrap();
    assert!(same_capture(&reference.envelope, &envelope));
    assert_eq!(reference.envelope.artifact, envelope.artifact);
    let sql = rusqlite::Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    let count: i64 = sql
        .query_row(
            "SELECT count(*) FROM acquisition_capture_links",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

/// Wait for an explicit durable-preparation message, not a timed sleep.
fn capture_writer(fixture: &Fixture) -> (OwnedChild, Handle) {
    fs::write(
        fixture.root.join("candidate.json"),
        serde_json::to_vec(&fixture.envelope).unwrap(),
    )
    .unwrap();
    fixture
        .db
        .release_source(&fixture.context.writer, fixture.context.now)
        .unwrap();
    let mut child = OwnedChild(
        Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "n37_crash::n37_capture_child_checkpoints_without_ack",
                "--nocapture",
            ])
            .env("MAESTRO_N37_CAPTURE_ROOT", fixture.root.as_os_str())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let reader = BufReader::new(child.0.stdout.take().unwrap());
    let capture = reader
        .lines()
        .map(Result::unwrap)
        .find_map(|line| {
            line.strip_prefix("N37_PREPARED ")
                .map(|handle| handle.parse().unwrap())
        })
        .unwrap();
    (child, capture)
}

#[test]
fn n37_owned_process_child() {
    if env::var("MAESTRO_N37_OWNED_CHILD").as_deref() != Ok("1") {
        return;
    }
    println!("N37_OWNED_READY");
    stdout().flush().unwrap();
    loop {
        thread::park();
    }
}

#[test]
fn n37_timeout_is_not_cancellation_and_stop_reaps_only_owned_child() {
    use super::n09_review_support::ManualClock;
    use maestro_acquisition::Refusal;
    use std::sync::Mutex;
    let launch = || {
        let mut child = OwnedChild(
            Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "n37_crash::n37_owned_process_child",
                    "--nocapture",
                ])
                .env("MAESTRO_N37_OWNED_CHILD", "1")
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let reader = BufReader::new(child.0.stdout.take().unwrap());
        assert!(
            reader
                .lines()
                .map(Result::unwrap)
                .any(|line| line == "N37_OWNED_READY")
        );
        child
    };
    let mut owned = launch();
    let mut foreign = launch();
    let cutoff = Instant::now();
    let frozen = ManualClock(Mutex::new(cutoff));
    let boundary = CountedClock {
        inner: &frozen,
        reads: AtomicUsize::new(0),
    };
    assert_eq!(
        stop_owned(&mut owned.0, cutoff, &boundary),
        Err(Refusal::Deadline)
    );
    assert_eq!(
        boundary.reads.load(Ordering::SeqCst),
        1,
        "expired budget entered cancellation loop"
    );
    assert!(
        owned.0.try_wait().unwrap().is_none(),
        "client deadline pretended cancellation"
    );
    let stopped = stop_owned(
        &mut owned.0,
        Instant::now() + Duration::from_secs(30),
        &HostClock,
    )
    .unwrap();
    assert!(!stopped.success());
    assert!(owned.0.try_wait().unwrap().is_some());
    assert!(
        foreign.0.try_wait().unwrap().is_none(),
        "foreign child stopped"
    );
    assert_eq!(
        stop_owned(&mut owned.0, Instant::now(), &HostClock).unwrap(),
        stopped,
        "reaping replay not idempotent"
    );
    stop_owned(
        &mut foreign.0,
        Instant::now() + Duration::from_secs(30),
        &HostClock,
    )
    .unwrap();
}
