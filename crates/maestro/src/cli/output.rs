//! How a command prints (plan D12): text for people by default; under
//! `--json`, one JSON document on stdout, its `schema` first. Diagnostics go
//! to stderr only, and a long command's job ID comes before anything else:
//! stdout's first line in text, stderr's under `--json`.

use crate::{
    failure::Failure,
    presentation::{
        message::Message,
        messages::{Interface, MessageKey, interpolate},
    },
};
use maestro_kernel::json::canonical;
use serde::Serialize;
use serde_json::Value;
use std::{
    env,
    io::{self, Write as _},
};
use ulid::Ulid;

/// How a command prints its result.
#[derive(Debug, Clone, Copy)]
pub(super) struct Output {
    /// Whether it prints one JSON document instead of text.
    json: bool,
    /// Explicit command-line no-colour choice.
    no_color: bool,
    /// Optional human interface selection; machine output always uses English.
    interface: Option<Interface>,
}

impl Output {
    /// Text for people, or one JSON document under `--json`.
    pub(super) fn new(json: bool) -> Self {
        Self {
            json,
            no_color: false,
            interface: None,
        }
    }

    /// Carry the global choice without touching preferences or planner output.
    pub(super) fn without_color(mut self, disabled: bool) -> Self {
        self.no_color = disabled;
        self
    }

    /// No-colour environment and incapable terminals retain all text information.
    pub(super) fn color(self) -> bool {
        !self.no_color
            && env::var_os("NO_COLOR").is_none()
            && env::var("TERM").is_ok_and(|term| {
                term != "dumb"
                    && term != "unknown"
                    && !term.contains("mono")
                    && !term.starts_with("vt")
            })
    }

    /// Bind validated session language once, displaying fallback only for human CLI.
    pub(super) fn with_language(self, language: &str) -> Result<Self, Failure> {
        self.with_language_to(language, |text| {
            diagnose(text);
            Ok(())
        })
    }

    /// Deliver interactive diagnostics through the selected presentation port.
    pub(super) fn with_language_to(
        mut self,
        language: &str,
        notice: impl FnOnce(&str) -> Result<(), Failure>,
    ) -> Result<Self, Failure> {
        if self.json {
            return Ok(self);
        }
        let interface = Interface::select(language).map_err(Failure::failed)?;
        self.interface = Some(interface);
        if interface.fallback() {
            notice(&self.wording(MessageKey::InterfaceFallback, &[("language", language)])?)?;
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

    /// Bind only a valid explicit repair-path language, without config reads or notices.
    pub(super) fn explicit_language(mut self, language: Option<&str>) -> Result<Self, Failure> {
        if !self.json {
            let language =
                language.and_then(|text| maestro_settings::canonical_language(text).ok());
            self.interface = Some(
                Interface::select(language.as_deref().unwrap_or("en")).map_err(Failure::failed)?,
            );
        }
        Ok(self)
    }

    /// Render an owned diagnostic at the human boundary; JSON always keeps English.
    pub(super) fn message(self, message: &Message) -> Result<String, Failure> {
        let interface = self
            .interface
            .map_or_else(|| Interface::select("en"), Ok)
            .map_err(Failure::failed)?;
        message.render(interface).map_err(Failure::failed)
    }

    /// Render typed failures only here; opaque downstream diagnostics remain literal data.
    pub(super) fn failure_text(self, failure: &Failure) -> Result<String, Failure> {
        match failure {
            Failure::RefusedMessage(message) | Failure::FailedMessage(message) => {
                self.message(message)
            }
            Failure::Refused(_) | Failure::Failed(_) => Ok(failure.to_string()),
        }
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
        .map_err(|error| {
            Failure::failed_message(Message::new(
                MessageKey::OutputWriteFailed,
                &[("error", &error.to_string())],
            ))
        })
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
    use std::{env, process::Command};

    #[test]
    fn output_color_checks_terminal_capability() {
        for (term, expected) in [("xterm", "true"), ("dumb", "false")] {
            let result = Command::new(env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "cli::output::tests::output_color_child_checks_capability",
                ])
                .env("TERM", term)
                .env("MAESTRO_EXPECT_COLOR", expected)
                .env_remove("NO_COLOR")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{term}: {}",
                String::from_utf8_lossy(&result.stdout)
            );
        }
    }

    #[test]
    #[ignore = "isolated environment probe run by output_color_checks_terminal_capability"]
    fn output_color_child_checks_capability() {
        let expected = env::var("MAESTRO_EXPECT_COLOR").unwrap() == "true";
        assert_eq!(Output::new(false).color(), expected);
        assert!(!Output::new(false).without_color(true).color());
    }

    #[test]
    fn output_preserves_its_json_mode() {
        assert!(!Output::new(false).is_json());
        assert!(Output::new(true).is_json());
    }
    #[test]
    fn catalog_presentation_typed_failures_keep_english_display_and_exit_codes() {
        use crate::{
            failure::Failure,
            presentation::{message::Message, messages::MessageKey},
        };
        use std::process::ExitCode;
        let message = || {
            Message::new(
                MessageKey::DiagnosticInstruction,
                &[("error", "English detail")],
            )
            .with_message(
                "instruction",
                Message::new(
                    MessageKey::TrustSuggestionCommand,
                    &[("path", "\"/synthetic/{literal}\"")],
                ),
            )
        };
        for language in ["en", "fr", "es"] {
            for tone in ["brief", "normal", "detailed"] {
                let output = Output::new(false).with_language(language).unwrap();
                let refused = Failure::refused_message(message());
                let failed = Failure::failed_message(message());
                assert_eq!(refused.code(), ExitCode::from(2), "{tone}");
                assert_eq!(failed.code(), ExitCode::from(1), "{tone}");
                let english = concat!(
                    "English detail; run: maestro trust add \"/synthetic/{literal}\" ",
                    "--confirm-path \"/synthetic/{literal}\""
                );
                assert_eq!(refused.to_string(), english);
                assert_eq!(failed.to_string(), english);
                assert_eq!(
                    Output::new(true)
                        .with_language(language)
                        .unwrap()
                        .failure_text(&refused)
                        .unwrap(),
                    english
                );
                let prefix = match language {
                    "fr" => "exécutez :",
                    "es" => "ejecute:",
                    _ => "run:",
                };
                assert_eq!(
                    output.failure_text(&refused).unwrap(),
                    english.replace("run:", prefix)
                );
                assert_eq!(
                    output
                        .failure_text(&Failure::failed("bare English detail"))
                        .unwrap(),
                    "bare English detail"
                );
            }
        }
    }
}
