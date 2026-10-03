//! `maestro doctor`: every check of the machine, each failure with its next
//! action, then what it found but must not touch. It neither creates nor
//! migrates the kernel, and deletes nothing; once the kernel's database
//! opens, it applies `config.toml`'s grants, as every command does. It exits
//! 0 when no check failed, a check that cannot run yet included, and 1 when
//! one failed.

use super::{
    check::Check,
    findings::{directory_findings, unreached_grants},
    graph_adapter as graph,
    kernel::{Opened, artifacts_check, bindings_check, config_check, database_check},
    services::{
        QDRANT_VARIABLE, ROUTER_VARIABLE, card_checks, qdrant_check, qdrant_url, router_check,
        router_url,
    },
    settings::settings_check,
};
use crate::{
    cli::{output::Output, setup},
    failure::Failure,
    settings::Session,
};
use maestro_kernel::paths::{self, Environment};
use serde::Serialize;
use std::{env, process::ExitCode};

/// The schema of the document `doctor` prints under `--json`.
const SCHEMA: &str = "maestro-cli/doctor/1";

/// How to handle database-creation temporary names.
const DATABASE_TEMPORARY_WARNING: &str = concat!(
    "may be another name of the live database; never open it; ",
    "inspect after no creation is in progress before removing only this name"
);

/// What `doctor` prints under `--json`.
#[derive(Debug, Serialize)]
struct DoctorDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// Every check, in the order they ran.
    checks: Vec<CheckDocument<'a>>,
    /// The entries of the data directory the kernel does not own, left as
    /// they are.
    untouched: Vec<String>,
    /// Database-creation temporary names, left as they are.
    database_temporaries: Vec<String>,
    /// How to handle database temporaries, or `null` when there are none.
    database_temporary_warning: Option<&'static str>,
    /// The grants of `config.toml` that reach no scope the kernel knows.
    unreached_grants: Vec<String>,
}

/// A check, as `doctor` prints it under `--json`.
#[derive(Debug, Serialize)]
struct CheckDocument<'a> {
    /// Its name.
    name: &'a str,
    /// What it looked at.
    target: &'a str,
    /// Whether it passed.
    passed: bool,
    /// Whether it ran: `false` for a check that cannot run yet, which
    /// neither passed nor failed.
    checked: bool,
    /// What it saw, or what is wrong.
    detail: &'a str,
    /// The next action, when it failed.
    next_action: Option<&'a str>,
}

/// Runs every check, the settings' with the `--set` flags `flags`, then
/// prints them with what doctor found but must not touch, and exits 1 when
/// a check failed.
///
/// # Errors
///
/// [`Failure::Failed`] when the kernel's directories cannot be resolved, or
/// its database, once it opened, cannot be read.
pub(in crate::cli) fn run(output: Output, flags: &[String]) -> Result<ExitCode, Failure> {
    let environment = Environment::current();
    let data = paths::data_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    let config_dir = paths::config_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    let (config, read) = config_check(&config_dir);
    let (database, opened) = database_check(&data, read.as_ref());
    let settings = settings_check(&config_dir, Session::for_cli(flags));
    let graph = graph::check(&environment, Session::for_cli(flags), read.as_ref());
    let mut checks = vec![
        config,
        settings,
        bindings_check(&config_dir),
        database,
        graph,
    ];
    checks.push(artifacts_check(&data, opened.as_ref()));
    checks.push(qdrant_check(
        &qdrant_url(env::var_os(QDRANT_VARIABLE).as_deref()),
        || setup::readiness(&environment, &setup::Tools::on_path()),
    ));
    checks.push(router_check(router_url(
        env::var_os(ROUTER_VARIABLE).as_deref(),
    )));
    checks.extend(card_checks());
    let (foreign_entries, temporary_entries) = directory_findings(&data);
    let untouched: Vec<String> = foreign_entries
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let database_temporaries: Vec<String> = temporary_entries
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let database_temporary_warning =
        (!database_temporaries.is_empty()).then_some(DATABASE_TEMPORARY_WARNING);
    let unreached: Vec<String> = match &opened {
        Some(Opened {
            database,
            scopes: Some(scopes),
        }) => unreached_grants(database, scopes)
            .map_err(|error| Failure::failed_by(&error))?
            .iter()
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    };
    let text = text(
        &checks,
        &untouched,
        (&database_temporaries, database_temporary_warning),
        &unreached,
    );
    let document = DoctorDocument {
        schema: SCHEMA,
        checks: checks.iter().map(check_document).collect(),
        untouched,
        database_temporaries,
        database_temporary_warning,
        unreached_grants: unreached,
    };
    output.result(&document, &text)?;
    Ok(exit_code(&checks))
}

/// 0 when no check failed, a check that cannot run yet included; else 1.
pub(super) fn exit_code(checks: &[Check]) -> ExitCode {
    if failures(checks) == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// How many checks failed.
fn failures(checks: &[Check]) -> usize {
    checks.iter().filter(|check| check.next().is_some()).count()
}

/// `check` as the document prints it.
fn check_document(check: &Check) -> CheckDocument<'_> {
    CheckDocument {
        name: check.name,
        target: &check.target,
        passed: check.is_checked() && check.next().is_none(),
        checked: check.is_checked(),
        detail: check.detail(),
        next_action: check.next(),
    }
}

/// The report for people: each check on a line, a failure's next action on
/// the line below it, then database temporaries, what doctor left untouched,
/// the grants that reach nothing, and how many checks failed or could not
/// run yet.
pub(super) fn text(
    checks: &[Check],
    untouched: &[String],
    temporary_names: (&[String], Option<&str>),
    unreached: &[String],
) -> String {
    let (database_temporaries, database_temporary_warning) = temporary_names;
    let mut lines = Vec::new();
    for check in checks {
        let verdict = match (check.is_checked(), check.next()) {
            (false, _) => "skip",
            (true, Some(_)) => "FAIL",
            (true, None) => "ok",
        };
        lines.push(format!(
            "{verdict:<5} {:<10} {}: {}",
            check.name,
            check.target,
            check.detail()
        ));
        lines.extend(check.next().map(|next| format!("      next: {next}")));
    }
    if let Some(warning) = database_temporary_warning {
        lines.push(format!(
            "Database temporary names ({warning}): {}",
            database_temporaries.join(", ")
        ));
    }
    if !untouched.is_empty() {
        lines.push(format!(
            "Not the kernel's, left untouched: {}",
            untouched.join(", ")
        ));
    }
    if !unreached.is_empty() {
        lines.push(format!(
            "Grants in config.toml that reach no known scope: {}",
            unreached.join(", ")
        ));
    }
    let failed = failures(checks);
    let unchecked = checks.iter().filter(|check| !check.is_checked()).count();
    lines.push(match (failed, unchecked) {
        (0, 0) => "Every check passed.".to_owned(),
        (0, _) => format!(
            "No check failed; {unchecked} of {} are not checked yet.",
            checks.len()
        ),
        _ => format!("{failed} of {} checks failed.", checks.len()),
    });
    lines.join("\n")
}
