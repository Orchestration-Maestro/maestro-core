//! Doctor's verdict: a check that cannot run yet, as each role's model card,
//! is shown as not checked, and fails neither the report nor the exit code.

use super::super::{
    check::Check,
    doctor::{exit_code, text},
    services::card_checks,
};
use std::process::ExitCode;

/// A passing check of every kernel file and service, then each role's
/// model card.
fn healthy_install() -> Vec<Check> {
    let mut checks: Vec<Check> = [
        "config",
        "bindings",
        "database",
        "artifacts",
        "qdrant",
        "router",
    ]
    .into_iter()
    .map(|name| Check::passed(name, "target", "fine"))
    .collect();
    checks.extend(card_checks());
    checks
}

#[test]
fn a_healthy_install_exits_0_with_the_model_cards_not_checked_yet() {
    let checks = healthy_install();
    assert_eq!(exit_code(&checks), ExitCode::SUCCESS);
    let report = text(&checks, &[], (&[], None), &[]);
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(lines.len(), 10, "{report}");
    assert!(lines[5].starts_with("ok    router "), "{report}");
    for (line, role) in lines[6..9].iter().zip(["embedder", "reranker", "answerer"]) {
        assert!(
            line.starts_with(&format!("skip  model_card {role}: not checked yet")),
            "{report}"
        );
    }
    assert_eq!(lines[9], "No check failed; 3 of 9 are not checked yet.");
}

#[test]
fn a_failed_check_still_exits_1_and_is_counted_alone() {
    let mut checks = healthy_install();
    checks[4] = Check::failed("qdrant", "target", "no answer", "start it");
    assert_eq!(exit_code(&checks), ExitCode::from(1));
    let report = text(&checks, &[], (&[], None), &[]);
    assert!(report.contains("FAIL  qdrant     target: no answer\n      next: start it\n"));
    assert!(report.ends_with("\n1 of 9 checks failed."), "{report}");
}

#[test]
fn every_check_passed_only_when_each_one_ran() {
    let checks = healthy_install();
    let ran = text(&checks[..6], &[], (&[], None), &[]);
    assert!(ran.ends_with("\nEvery check passed."), "{ran}");
}
