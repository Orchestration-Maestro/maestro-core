//! What the command line's tests share: a scratch home holding the kernel's
//! data and configuration directories, the public synthetic collection bound
//! there, the built `maestro` run in it under a deadline, and the kernel it
//! writes, opened beside it.

use maestro_kernel::{
    artifact::Digest,
    job::{Job, NewJob},
    journal::{Event, Filter},
    scope::{LOCAL, ScopeSet},
    store::Database,
};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{self, Child, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

/// How long a test waits for the binary to print a line or to end before it
/// kills it and fails: long enough for a slow CI host, and shorter than the
/// 20 s a mutation test is given at least, so that a binary that hangs fails
/// its test instead of timing out.
const DEADLINE: Duration = Duration::from_secs(15);

/// The kind of the job `knowledge import` runs.
pub(crate) const IMPORT: &str = "knowledge.import";
/// The resource an import of `synthetic` holds.
const SYNTHETIC_IMPORT: &str = "collection/synthetic/import";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: `data/` is `XDG_DATA_HOME` and
/// `config/` is `XDG_CONFIG_HOME` for the binary run in it.
pub(crate) struct Home(PathBuf);

impl Home {
    /// A home whose `config.toml` grants the local principal the whole
    /// default workspace, and whose `bindings.toml` binds `synthetic_root`
    /// to the public synthetic collection.
    pub(crate) fn new() -> Self {
        let home = Self::bare();
        home.configure("[access]\nread = ['workspace/default']\n");
        let binding = format!("synthetic_root = '{}'\n", synthetic().display());
        fs::write(home.config().join("bindings.toml"), binding).unwrap();
        home
    }

    /// A home with empty data and configuration directories: no grant, no
    /// binding.
    pub(crate) fn bare() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-cli-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("data").join("maestro")).unwrap();
        fs::create_dir_all(path.join("config").join("maestro")).unwrap();
        Self(path)
    }

    /// Writes `text` as the kernel's `config.toml`.
    pub(crate) fn configure(&self, text: &str) {
        fs::write(self.config().join("config.toml"), text).unwrap();
    }

    /// The home's own directory, for files a test writes beside the kernel.
    pub(crate) fn root(&self) -> &Path {
        &self.0
    }

    /// The kernel's data directory, `maestro` under `XDG_DATA_HOME`.
    pub(crate) fn data(&self) -> PathBuf {
        self.0.join("data").join("maestro")
    }

    /// The kernel's configuration directory, `maestro` under
    /// `XDG_CONFIG_HOME`.
    pub(crate) fn config(&self) -> PathBuf {
        self.0.join("config").join("maestro")
    }

    /// The binary with `arguments`, set to run in this home.
    pub(crate) fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
        command
            .args(arguments)
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Starts the binary with `arguments` in this home.
    pub(crate) fn start(&self, arguments: &[&str]) -> Running {
        Running::of(self.command(arguments))
    }

    /// Runs the binary with `arguments` in this home to its end.
    pub(crate) fn run(&self, arguments: &[&str]) -> Ended {
        self.start(arguments).finish()
    }

    /// Runs the binary with `arguments` in this home to its end, from the
    /// working directory `directory`.
    pub(crate) fn run_in(&self, directory: &Path, arguments: &[&str]) -> Ended {
        let mut command = self.command(arguments);
        command.current_dir(directory);
        Running::of(command).finish()
    }

    /// Adds the public synthetic collection, as its owner does first.
    pub(crate) fn add_synthetic(&self) {
        let declaration = synthetic().join("collection.json");
        let added = self.run(&[
            "knowledge",
            "collection",
            "add",
            declaration.to_str().unwrap(),
        ]);
        assert_eq!(added.code, Some(0), "{added:?}");
    }

    /// The kernel the binary wrote, opened beside it.
    pub(crate) fn database(&self) -> Database {
        Database::open_in(&self.data()).unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The public synthetic collection, `tests/fixtures/synthetic`.
pub(crate) fn synthetic() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("synthetic")
}

/// The frozen inputs `knowledge import --collection synthetic` submits its
/// job with: the collection, the digest of its declaration and that of its
/// one source's manifest, as the crate's documentation gives them.
pub(crate) fn synthetic_inputs() -> Value {
    let digest = |path: PathBuf| Digest::of(&fs::read(path).unwrap()).as_str().to_owned();
    let fixture = synthetic();
    json!({
        "collection": "synthetic",
        "declaration": digest(fixture.join("collection.json")),
        "manifests": {
            "handbook": digest(fixture.join("corpus").join("maestro-corpus.jsonl")),
        },
    })
}

/// Submits, in `database`, the job `knowledge import --collection synthetic`
/// would submit, as another process of the same command would.
pub(crate) fn submit_synthetic_import(database: &Database) -> Job {
    submit_import(database, &synthetic_inputs())
}

/// Submits, in `database`, an import of `synthetic` from other inputs, as
/// one of an older manifest would be: a job of another key, which holds the
/// same resource.
pub(crate) fn submit_other_import(database: &Database) -> Job {
    let older = json!({ "collection": "synthetic", "manifests": { "handbook": "older" } });
    submit_import(database, &older)
}

/// Submits, in `database`, an import of `synthetic` from `inputs`.
fn submit_import(database: &Database, inputs: &Value) -> Job {
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let new = NewJob {
        kind: IMPORT,
        inputs,
        scope: &scope,
        resource: Some(SYNTHETIC_IMPORT),
    };
    database.submit_job(&new, SystemTime::now()).unwrap()
}

/// What the local principal reads in `database`.
pub(crate) fn local(database: &Database) -> ScopeSet {
    database.visible(LOCAL).unwrap()
}

/// The events of `stream` the local principal reads, in order.
pub(crate) fn stream(database: &Database, stream: &str) -> Vec<Event> {
    let filter = Filter {
        stream,
        after: 0,
        r#type: None,
    };
    database.events(&local(database), &filter).unwrap()
}

/// The types of `events`, in order.
pub(crate) fn types(events: &[Event]) -> Vec<&str> {
    events.iter().map(|event| event.r#type.as_str()).collect()
}

/// The binary, running: its lines on stdout as it prints them, its stderr
/// once it ends.
pub(crate) struct Running {
    /// The process.
    child: Child,
    /// Each line it prints on stdout, as it prints it.
    lines: Receiver<String>,
    /// Everything it prints on stderr, once it closes it.
    stderr: Receiver<String>,
    /// The lines of stdout a test has read already.
    read: Vec<String>,
}

impl Running {
    /// Starts `command`, reading its two outputs on threads of their own so
    /// that neither pipe ever fills.
    fn of(mut command: Command) -> Self {
        let mut child = command.spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let (line, lines) = mpsc::channel();
        thread::spawn(move || {
            for text in BufReader::new(stdout).lines() {
                line.send(text.unwrap()).unwrap();
            }
        });
        let (text, errors) = mpsc::channel();
        thread::spawn(move || {
            let mut all = String::new();
            stderr.read_to_string(&mut all).unwrap();
            text.send(all).unwrap();
        });
        Self {
            child,
            lines,
            stderr: errors,
            read: Vec::new(),
        }
    }

    /// The next line the binary prints on stdout.
    pub(crate) fn line(&mut self) -> String {
        match self.lines.recv_timeout(DEADLINE) {
            Ok(line) => {
                self.read.push(line.clone());
                line
            }
            Err(error) => {
                let (ended, _) = self.end(Duration::ZERO);
                panic!("no line on stdout ({error}): {ended:?}");
            }
        }
    }

    /// Reads lines until one holds `text`, and returns it.
    pub(crate) fn line_with(&mut self, text: &str) -> String {
        loop {
            let line = self.line();
            if line.contains(text) {
                return line;
            }
        }
    }

    /// Waits for the binary to end, killing it once the deadline has passed,
    /// and returns what it did.
    pub(crate) fn finish(mut self) -> Ended {
        let (ended, killed) = self.end(DEADLINE);
        assert!(
            !killed,
            "the binary did not end within {DEADLINE:?}: {ended:?}"
        );
        ended
    }

    /// Waits `patience` for the binary to end, then kills it, and returns
    /// what it did and whether it had to be killed.
    fn end(&mut self, patience: Duration) -> (Ended, bool) {
        let deadline = Instant::now() + patience;
        let mut stdout = String::new();
        for line in &self.read {
            stdout.push_str(line);
            stdout.push('\n');
        }
        let mut killed = false;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    self.child.kill().unwrap();
                    killed = true;
                    // The rest, once the killed binary's stdout closes.
                    self.lines.iter().collect::<Vec<_>>().join("\n")
                }
            };
            stdout.push_str(&line);
            stdout.push('\n');
        }
        let stderr = self.stderr.recv().unwrap();
        let status = self.child.wait().unwrap();
        let ended = Ended {
            code: status.code(),
            stdout,
            stderr,
        };
        (ended, killed)
    }
}

impl Drop for Running {
    /// Kills the binary if it still runs, as when its test failed early, so
    /// that it never outlives the test's home.
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            drop(self.child.kill());
            drop(self.child.wait());
        }
    }
}

/// What the binary did, once it ended.
#[derive(Debug)]
pub(crate) struct Ended {
    /// Its exit code.
    pub(crate) code: Option<i32>,
    /// Everything it printed on stdout.
    pub(crate) stdout: String,
    /// Everything it printed on stderr.
    pub(crate) stderr: String,
}

impl Ended {
    /// The one JSON document it printed on stdout.
    pub(crate) fn json(&self) -> Value {
        let mut documents = serde_json::Deserializer::from_str(&self.stdout).into_iter::<Value>();
        let document = documents.next().unwrap().unwrap();
        assert!(
            documents.next().is_none(),
            "more than one document: {}",
            self.stdout
        );
        document
    }
}
