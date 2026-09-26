//! What the health tests share: a scratch directory for the kernel's data
//! and configuration, a stub HTTP server with one canned answer, an address
//! nothing answers at, and a failed check's parts.

use super::super::check::{Check, Outcome};
use std::{
    env, fs,
    io::{BufRead as _, BufReader, Write as _},
    net::TcpListener,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: `data` is the kernel's data
/// directory and `config` its configuration directory.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-cli-health-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("data")).unwrap();
        fs::create_dir_all(path.join("config")).unwrap();
        Self(path)
    }

    /// The kernel's data directory.
    pub(super) fn data(&self) -> PathBuf {
        self.0.join("data")
    }

    /// The kernel's configuration directory.
    pub(super) fn config(&self) -> PathBuf {
        self.0.join("config")
    }

    /// Writes `text` as `name` in the configuration directory.
    pub(super) fn configure(&self, name: &str, text: &str) {
        fs::write(self.config().join(name), text).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// A stub HTTP server on a new loopback port, on a thread of its own, that
/// answers every request with `status` and `body`; returns its address.
pub(super) fn serve(status: u16, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let body = body.to_owned();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            write!(
                &mut &stream,
                "HTTP/1.1 {status} Stub\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    address
}

/// An address on the loopback that nothing answers at: a port just freed.
pub(super) fn nothing_at() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

/// The problem and next action of `check`, which failed.
pub(super) fn failure(check: &Check) -> (&str, &str) {
    match &check.outcome {
        Outcome::Failed { problem, next } => (problem, next),
        Outcome::Passed(detail) => panic!("{} passed: {detail}", check.name),
    }
}

/// What `check` saw, which passed.
pub(super) fn detail(check: &Check) -> &str {
    match &check.outcome {
        Outcome::Passed(detail) => detail,
        Outcome::Failed { problem, .. } => panic!("{} failed: {problem}", check.name),
    }
}
