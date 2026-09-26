//! How a command prints (plan D12): text for people by default; under
//! `--json`, one JSON document on stdout, its `schema` first. Diagnostics go
//! to stderr only, and a long command's job ID comes before anything else:
//! stdout's first line in text, stderr's under `--json`.

use super::failure::Failure;
use serde::Serialize;
use std::io::{self, Write as _};
use ulid::Ulid;

/// How a command prints its result.
#[derive(Debug, Clone, Copy)]
pub(super) struct Output {
    /// Whether it prints one JSON document instead of text.
    json: bool,
}

impl Output {
    /// Text for people, or one JSON document under `--json`.
    pub(super) fn new(json: bool) -> Self {
        Self { json }
    }

    /// Prints `id`, the job a long command runs, before anything else: on
    /// stdout in text, on stderr under `--json`, so the document stays one.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when stdout cannot be written to.
    pub(super) fn job(self, id: Ulid) -> Result<(), Failure> {
        let line = format!("job {id}");
        if self.json {
            diagnose(&line);
            Ok(())
        } else {
            print(&line)
        }
    }

    /// Prints `line` for people; nothing under `--json`.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when stdout cannot be written to.
    pub(super) fn text(self, line: &str) -> Result<(), Failure> {
        if self.json { Ok(()) } else { print(line) }
    }

    /// Prints the command's result: `document` under `--json`, else `text`.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when stdout cannot be written to.
    pub(super) fn result(self, document: &impl Serialize, text: &str) -> Result<(), Failure> {
        if !self.json {
            return print(text);
        }
        let json = serde_json::to_string(document).map_err(|error| Failure::failed_by(&error))?;
        print(&json)
    }
}

/// Writes `line` and a line break on stdout, at once.
fn print(line: &str) -> Result<(), Failure> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::failed(format!("cannot write to stdout: {error}")))
}

/// Writes `line` and a line break on stderr, where diagnostics go; a stderr
/// that cannot be written to has nowhere else to say so.
pub(super) fn diagnose(line: &str) {
    let mut stderr = io::stderr().lock();
    drop(writeln!(stderr, "{line}"));
}
