//! How a command prints (plan D12): text for people by default; under
//! `--json`, one JSON document on stdout, its `schema` first. Diagnostics go
//! to stderr only, and a long command's job ID comes before anything else:
//! stdout's first line in text, stderr's under `--json`.

use crate::{
    failure::Failure,
    presentation::messages::{Interface, MessageKey, interpolate},
};
use maestro_kernel::json::canonical;
use serde::Serialize;
use serde_json::Value;
use std::io::{self, Write as _};
use ulid::Ulid;

/// How a command prints its result.
#[derive(Debug, Clone, Copy)]
pub(super) struct Output {
    /// Whether it prints one JSON document instead of text.
    json: bool,
    /// Optional human interface selection; machine output always uses English.
    interface: Option<Interface>,
}

impl Output {
    /// Text for people, or one JSON document under `--json`.
    pub(super) fn new(json: bool) -> Self {
        Self {
            json,
            interface: None,
        }
    }

    /// Bind validated session language once, displaying fallback only for human CLI.
    pub(super) fn with_language(mut self, language: &str) -> Result<Self, Failure> {
        if self.json {
            return Ok(self);
        }
        let interface = Interface::select(language).map_err(Failure::failed)?;
        self.interface = Some(interface);
        if interface.fallback() {
            diagnose(&self.wording(MessageKey::InterfaceFallback, &[("language", language)])?);
        }
        Ok(self)
    }

    /// Select interface prose, then insert literal data. JSON diagnostics stay English.
    pub(super) fn wording(
        self,
        key: MessageKey,
        values: &[(&str, &str)],
    ) -> Result<String, Failure> {
        let interface = match self.interface {
            Some(interface) => interface,
            _ => Interface::select("en").map_err(Failure::failed)?,
        };
        interpolate(interface.template(key), values).map_err(Failure::failed)
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

    /// Whether it prints one JSON document instead of text.
    pub(super) const fn is_json(self) -> bool {
        self.json
    }

    /// Prints `line` for people; nothing under `--json`.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when stdout cannot be written to.
    pub(super) fn text(self, line: &str) -> Result<(), Failure> {
        if self.json { Ok(()) } else { print(line) }
    }

    /// Prints a foreground job's step with the historic sorted JSON bytes.
    ///
    /// # Errors
    /// [`Failure::Failed`] when stdout cannot be written to.
    pub(super) fn step(self, data: &Value) -> Result<(), Failure> {
        self.text(&format!("step {}", canonical(data.clone())))
    }

    /// Prints a refusal document as JSON, or its diagnostic on stderr for people.
    pub(super) fn refusal(
        self,
        document: &impl Serialize,
        diagnostic: &str,
    ) -> Result<(), Failure> {
        if self.json {
            self.result(document, "")
        } else {
            diagnose(diagnostic);
            Ok(())
        }
    }

    /// Prints an opaque JSON document in the pre-Cedar sorted-object order.
    /// Typed documents must use `result` to keep their declared field order.
    ///
    /// # Errors
    /// [`Failure::Failed`] when stdout cannot be written to.
    pub(super) fn json_result(self, document: &Value, text: &str) -> Result<(), Failure> {
        self.result(&canonical(document.clone()), text)
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

#[cfg(test)]
mod tests {
    use super::Output;

    #[test]
    fn output_preserves_its_json_mode() {
        assert!(!Output::new(false).is_json());
        assert!(Output::new(true).is_json());
    }
}
