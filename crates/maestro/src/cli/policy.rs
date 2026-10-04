//! Effect-free authoring policy checks and synthetic neighbour tests.
//! No stdin, flag or authoring file supplies production host authority.

use super::output::Output;
use crate::failure::Failure;
use clap::Subcommand;
use maestro_catalog::{
    limits::Limits,
    policy::{
        Case, Check, Decision, HostFacts, NoFacts, Operation, PolicyChecker, TrustedFacts, load,
        read_input, test_cases,
    },
};
use serde::Serialize;
use std::{io, path::PathBuf, process::ExitCode};

/// Authoring policy commands, never operation execution.
#[derive(Debug, Subcommand)]
pub(super) enum PolicyCommand {
    /// Check normalized operation data; trusted host facts require C20.
    Check {
        /// Read one bounded operation JSON object from standard input.
        #[arg(long, required = true)]
        stdin: bool,
        /// Authoring catalog directory, not installed policy authority.
        #[arg(long, value_name = "DIR")]
        catalog_dir: PathBuf,
    },
    /// Run synthetic allow/deny neighbours, executing no requested effects.
    Test {
        /// Authoring catalog directory.
        #[arg(long, value_name = "DIR")]
        catalog_dir: PathBuf,
    },
}

/// Test-only host adapter for an authoring stimulus, never used by check.
struct CaseHost<'a>(&'a Case);

impl HostFacts for CaseHost<'_> {
    fn facts(&self, _operation: &Operation) -> Option<TrustedFacts> {
        self.0.facts.clone()
    }
}

/// Machine-readable authoring test summary.
#[derive(Serialize)]
struct TestDocument {
    /// Fixed schema.
    schema: &'static str,
    /// Whether every stimulus matched its expected outcome.
    status: &'static str,
    /// Cases that matched their expectations.
    passed: usize,
    /// Every failed case or loading diagnostic.
    diagnostics: Vec<String>,
}

/// Checks every stimulus, retaining every mismatch rather than stopping early.
fn check_cases(checker: &dyn PolicyChecker, cases: &[Case]) -> TestDocument {
    let mut document = TestDocument {
        schema: "maestro-cli/policy-test/1",
        status: "passed",
        passed: 0,
        diagnostics: Vec::new(),
    };
    for case in cases {
        let checked = checker.check(&case.operation, &CaseHost(case));
        if checked.decision == case.expected {
            document.passed += 1;
        } else {
            document.diagnostics.push(format!(
                "{}: expected {:?}, got {:?}: {}",
                case.name,
                case.expected,
                checked.decision,
                checked.diagnostics.join("; ")
            ));
        }
    }
    document
}

/// Runs a check or authoring test without opening the kernel or an executor.
pub(super) fn run(output: Output, command: &PolicyCommand) -> Result<ExitCode, Failure> {
    let limits = &Limits::PRODUCTION;
    match command {
        PolicyCommand::Check {
            catalog_dir,
            stdin: _,
        } => {
            let result = load(catalog_dir, limits)
                .and_then(|checker| {
                    let text = read_input(io::stdin().lock(), limits)?;
                    let operation = Operation::parse(&text, limits)?;
                    Ok(checker.check(&operation, &NoFacts))
                })
                .unwrap_or_else(Check::denied);
            let text = format!(
                "policy check: {:?}\n{}",
                result.decision,
                result.diagnostics.join("\n")
            );
            let allowed = result.decision == Decision::Allow;
            if allowed {
                output.result(&result, "policy check: allow")?;
            } else {
                output.refusal(&result, &text)?;
            }
            Ok(if allowed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
        PolicyCommand::Test { catalog_dir } => {
            let result = load(catalog_dir, limits).and_then(|checker| {
                let cases = test_cases(catalog_dir, limits)?;
                Ok(check_cases(&checker, &cases))
            });
            let mut document = result.unwrap_or_else(|error| TestDocument {
                schema: "maestro-cli/policy-test/1",
                status: "refused",
                passed: 0,
                diagnostics: vec![error],
            });
            if document.diagnostics.is_empty() {
                output.result(
                    &document,
                    &format!("policy test passed: {} cases", document.passed),
                )?;
                Ok(ExitCode::SUCCESS)
            } else {
                document.status = "refused";
                output.refusal(&document, &document.diagnostics.join("\n"))?;
                Ok(ExitCode::from(2))
            }
        }
    }
}
