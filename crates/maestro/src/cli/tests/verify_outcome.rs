//! Verification findings make the verify job fail without dropping its report.

use crate::cli::verify::outcome;
use maestro_kernel::job::JobState;
use maestro_knowledge::publish::Verification;
use serde_json::json;

/// A report with the findings under test.
fn report(findings: Vec<String>) -> Verification {
    Verification {
        collection: "synthetic".to_owned(),
        generation: 3,
        chunk_set: "prepared".to_owned(),
        point_count: 2,
        findings,
    }
}

#[test]
fn no_findings_succeed_and_return_the_complete_report() {
    let (state, result) = outcome(&report(Vec::new()));
    assert_eq!(state, JobState::Succeeded);
    assert_eq!(result["findings"], json!([]));
    assert_eq!(result["generation"], 3);
}

#[test]
fn findings_fail_and_remain_named_in_the_report() {
    let findings = vec!["missing artifact of chunk 1".to_owned()];
    let (state, result) = outcome(&report(findings.clone()));
    assert_eq!(state, JobState::Failed);
    assert_eq!(result["findings"], json!(findings));
}
