//! The embedded graph's check: off with no probe at all, a selected engine
//! the build lacks, the graph directory's problems, and the files a fake
//! projection receipt publishes, each refused outside the graph directory or
//! through a link before any open, then opened read-only, queried, closed,
//! reopened and queried again. Nothing is ever created or changed.

use super::{
    super::{
        check::{Check, Outcome},
        graph::check_with,
    },
    support::{Scratch, failure},
};
use crate::{cli::session, failure::Failure, settings::Session};
use maestro_catalog::settings::NoWorkspaceTrust;
use maestro_kernel::paths::Environment;
use maestro_knowledge::graph::projection::health::{
    OpenGraph, ProbeError, PublishedFile, PublishedGraph, Receipt,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

/// One step a fake engine took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// A read-only open, which succeeded.
    Open,
    /// A `RETURN 1` query.
    Query,
    /// A close, when the open file was dropped.
    Close,
}

/// A published file whose opens and queries answer in turn from scripts,
/// then succeed, recording every step it took.
struct FakeFile<'a> {
    /// Its path.
    path: PathBuf,
    /// What its next opens answer.
    opens: RefCell<VecDeque<Result<(), ProbeError>>>,
    /// What its next queries answer.
    queries: RefCell<VecDeque<Result<(), ProbeError>>>,
    /// The steps taken, shared by every file of a receipt.
    steps: &'a RefCell<Vec<Step>>,
}

impl<'a> FakeFile<'a> {
    fn new(path: PathBuf, steps: &'a RefCell<Vec<Step>>) -> Self {
        Self {
            path,
            opens: RefCell::default(),
            queries: RefCell::default(),
            steps,
        }
    }

    fn opens(self, answers: &[Result<(), &str>]) -> Self {
        self.opens.replace(scripted(answers));
        self
    }

    fn queries(self, answers: &[Result<(), &str>]) -> Self {
        self.queries.replace(scripted(answers));
        self
    }
}

/// What a fake file's next opens or queries answer, in turn.
type Script<'a> = &'a [Result<(), &'a str>];

/// `answers` as a script.
fn scripted(answers: &[Result<(), &str>]) -> VecDeque<Result<(), ProbeError>> {
    answers
        .iter()
        .map(|answer| answer.map_err(|message| ProbeError::Unreadable(message.to_owned())))
        .collect()
}

impl PublishedFile for &FakeFile<'_> {
    fn path(&self) -> &Path {
        &self.path
    }

    fn open_read_only(&self) -> Result<Box<dyn OpenGraph + '_>, ProbeError> {
        self.opens.borrow_mut().pop_front().unwrap_or(Ok(()))?;
        self.steps.borrow_mut().push(Step::Open);
        Ok(Box::new(FakeOpen(self)))
    }
}

/// A fake file, open.
struct FakeOpen<'a, 'b>(&'a FakeFile<'b>);

impl OpenGraph for FakeOpen<'_, '_> {
    fn query_one(&self) -> Result<(), ProbeError> {
        self.0.steps.borrow_mut().push(Step::Query);
        self.0
            .queries
            .borrow_mut()
            .pop_front()
            .unwrap_or(Ok(()))
            .map_err(|error| ProbeError::Corrupt(format!("{error:?}")))
    }
}

impl Drop for FakeOpen<'_, '_> {
    fn drop(&mut self) {
        self.0.steps.borrow_mut().push(Step::Close);
    }
}

/// A receipt that publishes `files`, none when `None`, or is unreadable,
/// counting how often it was read.
struct FakeReceipt<'a> {
    /// What it publishes, or why it cannot be read.
    files: Result<Option<Vec<FakeFile<'a>>>, String>,
    /// How often it was read.
    reads: Cell<usize>,
}

impl<'a> FakeReceipt<'a> {
    fn publishing(files: Vec<FakeFile<'a>>) -> Self {
        Self {
            files: Ok(Some(files)),
            reads: Cell::new(0),
        }
    }
}

impl PublishedGraph for FakeReceipt<'_> {
    fn receipt(&self) -> Result<Receipt<'_>, ProbeError> {
        self.reads.set(self.reads.get() + 1);
        match &self.files {
            Err(_) => Err(ProbeError::InventoryUnreadable),
            Ok(None) => Ok(Receipt::NonePublished),
            Ok(Some(files)) => Ok(Receipt::Files(
                files
                    .iter()
                    .map(|file| Box::new(file) as Box<dyn PublishedFile + '_>)
                    .collect(),
            )),
        }
    }
}

/// A scratch home whose data directory's graph directory the tests shape.
struct Home {
    /// The scratch directory.
    scratch: Scratch,
    /// Its environment: the scratch data directory as `XDG_DATA_HOME`.
    environment: Environment,
}

impl Home {
    fn new() -> Self {
        let scratch = Scratch::new();
        let mut environment = Environment::default();
        environment.xdg_data_home = Some(scratch.data().into_os_string());
        Self {
            scratch,
            environment,
        }
    }

    /// A home whose graph directory exists, its owner's alone.
    fn ready() -> Self {
        let home = Self::new();
        fs::create_dir_all(home.graph()).unwrap();
        make_private(&home.graph());
        home
    }

    fn graph(&self) -> PathBuf {
        self.scratch.data().join("maestro").join("graph")
    }

    /// A file of the graph directory holding `text`.
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.graph().join(name);
        fs::write(&path, text).unwrap();
        path
    }

    /// The session with `flags`.
    fn session(&self, flags: &[&str]) -> Result<Session, Failure> {
        let flags: Vec<String> = flags.iter().map(|&flag| flag.to_owned()).collect();
        session::health_at(
            &self.scratch.config(),
            None,
            None,
            &flags,
            &NoWorkspaceTrust,
        )
    }

    /// The check with `graph.engine = lbug`, in a build with the engine.
    fn check(&self, published: &dyn PublishedGraph) -> Check {
        check_with(
            &self.environment,
            self.session(&["graph.engine=ladybug"])
                .as_ref()
                .map_err(|error| Failure::refused(error.to_string())),
            true,
            published,
        )
    }
}

/// Gives `directory` mode `0700` on Unix.
fn make_private(directory: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = directory;
}

/// Why `check` did not run.
fn not_checked(check: &Check) -> &str {
    let Outcome::NotChecked(reason) = &check.outcome else {
        panic!("{check:?}");
    };
    reason
}

#[test]
fn an_off_graph_reads_no_receipt_and_creates_nothing() {
    let home = Home::new();
    let receipt = FakeReceipt::publishing(Vec::new());
    for engine_built in [false, true] {
        let check = check_with(
            &home.environment,
            home.session(&[])
                .as_ref()
                .map_err(|error| Failure::refused(error.to_string())),
            engine_built,
            &receipt,
        );
        assert_eq!(
            not_checked(&check),
            "the graph is off (graph.engine = none)"
        );
        assert_eq!(check.target, home.graph().display().to_string());
    }
    assert_eq!(receipt.reads.get(), 0);
    assert!(!home.graph().exists());
}

#[test]
fn ladybug_in_a_build_without_the_engine_fails_before_any_probe() {
    let home = Home::ready();
    let receipt = FakeReceipt::publishing(Vec::new());
    let check = check_with(
        &home.environment,
        home.session(&["graph.engine=ladybug"])
            .as_ref()
            .map_err(|error| Failure::refused(error.to_string())),
        false,
        &receipt,
    );
    let (problem, next) = failure(&check);
    assert!(problem.contains("built without the engine"), "{problem}");
    assert!(next.contains("`engine` feature"), "{next}");
    assert_eq!(receipt.reads.get(), 0);
}

#[test]
fn unresolved_directories_and_settings_fail_with_their_next_action() {
    let home = Home::new();
    let receipt = FakeReceipt::publishing(Vec::new());
    let unresolved = check_with(
        &Environment::default(),
        home.session(&[])
            .as_ref()
            .map_err(|error| Failure::refused(error.to_string())),
        true,
        &receipt,
    );
    assert_eq!(unresolved.target, "data directory");
    assert!(failure(&unresolved).1.contains("XDG_DATA_HOME"));
    let refused = check_with(
        &home.environment,
        Err(Failure::refused("unknown key graph.path")),
        true,
        &receipt,
    );
    assert_eq!(failure(&refused).0, "unknown key graph.path");
    assert_eq!(receipt.reads.get(), 0);
}

#[test]
fn a_missing_or_misplaced_graph_directory_is_named_and_left_alone() {
    let home = Home::new();
    let receipt = FakeReceipt::publishing(Vec::new());
    let missing = home.check(&receipt);
    assert_eq!(
        failure(&missing),
        (
            "the graph directory is missing",
            "run `maestro setup --yes`, which creates it"
        )
    );
    assert!(!home.graph().exists());
    fs::create_dir_all(home.graph().parent().unwrap()).unwrap();
    fs::write(home.graph(), "kept").unwrap();
    let misplaced = home.check(&receipt);
    assert_eq!(failure(&misplaced).0, "the graph path is not a directory");
    assert_eq!(fs::read_to_string(home.graph()).unwrap(), "kept");
    assert_eq!(receipt.reads.get(), 0);
}

#[cfg(unix)]
#[test]
fn shared_permissions_and_a_relocated_directory_are_reported_unchanged() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let home = Home::ready();
    let receipt = FakeReceipt::publishing(Vec::new());
    fs::set_permissions(home.graph(), fs::Permissions::from_mode(0o750)).unwrap();
    let shared = home.check(&receipt);
    assert!(failure(&shared).0.contains("permissions"), "{shared:?}");
    assert!(failure(&shared).1.contains("0700"), "{shared:?}");
    let mode = fs::metadata(home.graph()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o750);

    let moved = home.scratch.data().join("moved");
    fs::rename(home.graph(), &moved).unwrap();
    symlink(&moved, home.graph()).unwrap();
    let relocated = home.check(&receipt);
    assert!(failure(&relocated).0.contains("relocated"), "{relocated:?}");
    assert!(fs::symlink_metadata(home.graph()).unwrap().is_symlink());
    assert_eq!(receipt.reads.get(), 0);
}

#[test]
fn until_a_graph_is_published_nothing_is_opened() {
    let home = Home::ready();
    let check = home.check(&FakeReceipt {
        files: Ok(None),
        reads: Cell::new(0),
    });
    assert_eq!(
        not_checked(&check),
        "no graph is published yet, so no file was opened"
    );
    assert_eq!(fs::read_dir(home.graph()).unwrap().count(), 0);
}

#[test]
fn an_unreadable_or_empty_receipt_fails_without_an_open() {
    let home = Home::ready();
    let unreadable = FakeReceipt {
        files: Err("no receipt row".to_owned()),
        reads: Cell::new(0),
    };
    let check = home.check(&unreadable);
    assert_eq!(failure(&check).0, "the projection receipt cannot be read");
    let empty = home.check(&FakeReceipt::publishing(Vec::new()));
    assert_eq!(
        failure(&empty).0,
        "the projection receipt publishes no file"
    );
    assert!(failure(&empty).1.contains("rebuild the projection"));
}

#[test]
fn each_published_file_opens_read_only_answers_closes_and_reopens() {
    let home = Home::ready();
    let steps = RefCell::default();
    fs::create_dir(home.graph().join("nested")).unwrap();
    let first = home.file("one.lbug", "");
    let second = home.file("nested/two.lbug", "");
    let receipt = FakeReceipt::publishing(vec![
        FakeFile::new(first.clone(), &steps),
        FakeFile::new(second.clone(), &steps),
    ]);
    let check = home.check(&receipt);
    let Outcome::Passed(detail) = &check.outcome else {
        panic!("{check:?}");
    };
    for path in [&first, &second] {
        assert!(detail.contains(&path.display().to_string()), "{detail}");
    }
    assert!(detail.contains("opened read-only in"), "{detail}");
    assert!(detail.contains("answered both times"), "{detail}");
    let once = [Step::Open, Step::Query, Step::Close];
    assert_eq!(*steps.borrow(), [once, once, once, once].concat());
    assert_eq!(check.target, home.graph().display().to_string());
}

#[test]
fn graph_health_refuses_outside_and_missing_inside_files_before_any_open() {
    let home = Home::ready();
    let steps = RefCell::default();
    let authority = home.scratch.data().join("maestro").join("kernel.sqlite3");
    fs::write(&authority, "authority").unwrap();
    fs::create_dir(home.graph().join("directory")).unwrap();
    for (path, problem) in [
        (authority.clone(), "outside the graph directory"),
        (
            home.graph().join("..").join("kernel.sqlite3"),
            "not plain names",
        ),
        (home.graph(), "the graph directory itself"),
        (home.graph().join("directory"), "is not a file"),
        (
            home.graph().join("absent.lbug"),
            "the receipt-named graph file is missing",
        ),
    ] {
        let receipt = FakeReceipt::publishing(vec![FakeFile::new(path.clone(), &steps)]);
        let check = home.check(&receipt);
        assert!(failure(&check).0.contains(problem), "{path:?}: {check:?}");
        assert!(failure(&check).1.contains("keep the file"), "{check:?}");
        assert_eq!(check.target, path.display().to_string());
    }
    assert!(steps.borrow().is_empty());
    assert_eq!(fs::read_to_string(authority).unwrap(), "authority");
}

#[cfg(unix)]
#[test]
fn a_linked_file_or_directory_on_the_way_is_refused_before_any_open() {
    use std::os::unix::fs::symlink;
    let home = Home::ready();
    let steps = RefCell::default();
    let outside = home.scratch.data().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("graph.lbug"), "kept").unwrap();
    symlink(outside.join("graph.lbug"), home.graph().join("file.lbug")).unwrap();
    symlink(&outside, home.graph().join("linked")).unwrap();
    for path in [
        home.graph().join("file.lbug"),
        home.graph().join("linked").join("graph.lbug"),
    ] {
        let receipt = FakeReceipt::publishing(vec![FakeFile::new(path.clone(), &steps)]);
        let check = home.check(&receipt);
        assert!(failure(&check).0.ends_with("is a link"), "{check:?}");
    }
    assert!(steps.borrow().is_empty());
    assert_eq!(
        fs::read_to_string(outside.join("graph.lbug")).unwrap(),
        "kept"
    );
}

#[test]
fn a_writer_s_lock_on_open_or_reopen_says_to_wait() {
    let home = Home::ready();
    let path = home.file("graph.lbug", "");
    // G25's messages: Windows refuses a reader beside a writer.
    for opens in [
        &[Err("another process has locked a portion of the file")][..],
        &[Ok(()), Err("Could not set lock on file")],
    ] {
        let steps = RefCell::default();
        let receipt =
            FakeReceipt::publishing(vec![FakeFile::new(path.clone(), &steps).opens(opens)]);
        for open in receipt.files.as_ref().unwrap().as_ref().unwrap()[0]
            .opens
            .borrow_mut()
            .iter_mut()
        {
            if let Err(ProbeError::Unreadable(message)) = open {
                *open = Err(ProbeError::Locked(message.clone()));
            }
        }
        let check = home.check(&receipt);
        let (problem, next) = failure(&check);
        assert!(problem.starts_with("a writer holds"), "{problem}");
        assert_eq!(
            next,
            "let the writer finish, then run `maestro doctor` again"
        );
        assert_eq!(check.target, path.display().to_string());
    }
}

#[test]
fn a_file_that_does_not_open_or_answer_is_to_be_rebuilt_never_repaired() {
    let home = Home::ready();
    let path = home.file("graph.lbug", "not a graph");
    let cases: [(Script<'_>, Script<'_>, &str); 3] = [
        (
            &[Err("not a valid database file")],
            &[],
            "does not open read-only",
        ),
        (&[], &[Err("checksum mismatch")], "does not answer"),
        (&[], &[Ok(()), Err("checksum mismatch")], "does not answer"),
    ];
    for (opens, queries, problem) in cases {
        let steps = RefCell::default();
        let file = FakeFile::new(path.clone(), &steps)
            .opens(opens)
            .queries(queries);
        let check = home.check(&FakeReceipt::publishing(vec![file]));
        assert!(failure(&check).0.contains(problem), "{check:?}");
        assert!(failure(&check).1.starts_with("keep the files"), "{check:?}");
        let steps = steps.borrow();
        let count = |step| steps.iter().filter(|&&taken| taken == step).count();
        assert_eq!(
            count(Step::Open),
            count(Step::Close),
            "every open is closed"
        );
    }
    assert_eq!(fs::read_to_string(path).unwrap(), "not a graph");
}

#[test]
fn the_first_failing_file_stops_the_probe() {
    let home = Home::ready();
    let steps = RefCell::default();
    let broken = home.file("broken.lbug", "");
    let receipt = FakeReceipt::publishing(vec![
        FakeFile::new(broken, &steps).opens(&[Err("not a valid database file")]),
        FakeFile::new(home.file("next.lbug", ""), &steps),
    ]);
    let check = home.check(&receipt);
    assert!(failure(&check).0.contains("does not open"), "{check:?}");
    assert!(
        steps.borrow().is_empty(),
        "the second file was never opened"
    );
    assert_eq!(receipt.reads.get(), 1);
}

#[test]
fn uncaptured_lock_words_are_unreadable_not_writer_contention() {
    let home = Home::ready();
    let steps = RefCell::default();
    for message in [
        "clock failure",
        "deadlock",
        "unknown lock error",
        "Cannot open file block.lbdb",
    ] {
        let file = FakeFile::new(home.file("clock.lbug", "kept"), &steps).opens(&[Err(message)]);
        let check = home.check(&FakeReceipt::publishing(vec![file]));
        assert!(
            failure(&check).0.contains("corrupt or unreadable"),
            "{check:?}"
        );
        assert!(failure(&check).1.contains("rebuild"));
    }
    assert!(steps.borrow().is_empty());
}
