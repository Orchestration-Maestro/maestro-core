//! The check on the suites and corpus this machine names, by hand:
//! `MAESTRO_SUITE_DIRECTORY` names a directory of suites, and
//! `MAESTRO_CORPUS_MANIFEST` a corpus manifest, `maestro-corpus/1`, whose
//! files the check reads, every one when an unanswerable question has
//! identifier-like terms, so a release build is quicker on a large corpus.
//! It prints the counts of each suite, the documents whose file changed since
//! its manifest line, every problem, and the leads of the unanswerable
//! questions with the questions not probed, as JSON, and fails on a problem,
//! never on a lead: an explicit local run, never a pass when ignored. It
//! reads the corpus, not a kernel: a document the quality gate holds back
//! shows up only when the runner resolves the suite in a generation, after
//! the import and the gate.

use super::check::check;
use serde_json::json;
use std::{env, path::Path};

/// The value of `variable`, without which the check cannot start.
fn required(variable: &str) -> String {
    env::var(variable)
        .unwrap_or_else(|_| panic!("set {variable}; see tests/it/suite_check/local_suites.rs"))
}

#[test]
#[ignore = "checks the suites and corpus this machine names: MAESTRO_SUITE_DIRECTORY=<directory> \
            MAESTRO_CORPUS_MANIFEST=<maestro-corpus.jsonl> cargo test --release -p \
            maestro-knowledge --test it suite_check::local_suites -- --ignored --nocapture"]
fn every_name_of_the_local_suites_gives_one_section_or_document() {
    let suites = required("MAESTRO_SUITE_DIRECTORY");
    let manifest = required("MAESTRO_CORPUS_MANIFEST");
    let checked = check(Path::new(&suites), Path::new(&manifest));
    let suites: serde_json::Map<String, serde_json::Value> = checked
        .suites
        .iter()
        .map(|(name, counts)| {
            let counts = json!({
                "questions": counts.questions,
                "unanswerable": counts.unanswerable,
                "sections": counts.sections,
                "documents": counts.documents,
            });
            (name.clone(), counts)
        })
        .collect();
    let leads: serde_json::Map<String, serde_json::Value> = checked
        .unanswerable_leads
        .iter()
        .map(|(question, lead)| {
            let lead = json!({"terms": lead.terms, "documents": lead.documents});
            (question.clone(), lead)
        })
        .collect();
    let report = json!({
        "suites": suites,
        "changed_documents": checked.changed,
        "problems": checked.problems,
        "unanswerable_leads": leads,
        "not_probed": checked.not_probed,
    });
    println!("{report:#}");
    assert!(
        checked.problems.is_empty(),
        "{} problems",
        checked.problems.len()
    );
}
