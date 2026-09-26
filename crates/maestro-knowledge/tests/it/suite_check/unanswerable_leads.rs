//! Leads for unanswerable questions: each document of the manifest that holds
//! every identifier-like term of an unanswerable question, the term the
//! lexical analyzer gives a word with a digit, a joiner or a camelCase part,
//! is a lead that a person reads before the suite is frozen, since it may
//! answer the question; a term no document holds gives none, and a question
//! without such a term is not probed.

use super::{
    check::Lead,
    scratch::{ROTATION, ROTATION_REF, Scratch, line, name, question},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A document of error codes, with sections.
const ERRORS: &str = "# Errors\n\n## ERR-4012\n\nThe job stopped at max_retries.\n\n\
    ## ERR-4013\n\nThe agent at AgentPort 7005 did not answer.\n";
/// The `source_ref` of [`ERRORS`].
const ERRORS_REF: &str = "corpus-path:errors.md";

/// The unanswerable question `id`, asked as `text`.
fn unanswerable(id: &str, text: &str) -> Value {
    json!({
        "schema": "maestro-suite/1", "id": id, "language": "en", "question": text,
        "answerable": false, "expected": [],
    })
}

/// A scratch corpus of [`ROTATION`] and [`ERRORS`], each declared truly, and
/// the suite `leads`: one answerable question, then `questions`.
fn asked(questions: &[Value]) -> Scratch {
    let scratch = Scratch::new();
    scratch.document("rotation.md", ROTATION.as_bytes());
    scratch.document("errors.md", ERRORS.as_bytes());
    scratch.manifest(&[
        line("rotation.md", ROTATION.as_bytes(), ROTATION_REF),
        line("errors.md", ERRORS.as_bytes(), ERRORS_REF),
    ]);
    let found = question("found", &json!([name(ROTATION_REF, &["Rotation"])]));
    let lines: Vec<Value> = [found].into_iter().chain(questions.to_vec()).collect();
    scratch.suite("leads", &lines);
    scratch
}

/// The lead of the terms `terms` to the documents `documents`.
fn lead(terms: &[&str], documents: &[&str]) -> Lead {
    Lead {
        terms: terms.iter().map(|&term| term.to_owned()).collect(),
        documents: documents
            .iter()
            .map(|&document| document.to_owned())
            .collect(),
    }
}

#[test]
fn a_document_holding_every_identifier_term_of_an_unanswerable_question_is_a_lead() {
    let scratch = asked(&[
        unanswerable("code", "Does ERR-4012 reset max_retries?"),
        unanswerable("port", "Can AgentPort be moved off 7005?"),
        unanswerable("escrow", "Where is the escrow copy of the key kept?"),
    ]);
    let checked = scratch.check();
    assert_eq!(checked.problems, Vec::<String>::new());
    let leads = BTreeMap::from([
        (
            "leads: code".to_owned(),
            lead(&["err-4012", "max_retries"], &[ERRORS_REF]),
        ),
        (
            "leads: port".to_owned(),
            lead(&["7005", "agentport"], &[ERRORS_REF]),
        ),
    ]);
    assert_eq!(checked.unanswerable_leads, leads);
    assert_eq!(checked.not_probed, ["leads: escrow"]);
}

#[test]
fn no_lead_comes_of_an_absent_term_or_of_terms_held_apart() {
    // ERR-9999 is nowhere; ERR-4012 and 50 MiB are, but in two documents.
    let scratch = asked(&[
        unanswerable("absent", "What does ERR-9999 mean?"),
        unanswerable("apart", "Does ERR-4012 happen at 50 MiB?"),
    ]);
    let checked = scratch.check();
    assert_eq!(checked.problems, Vec::<String>::new());
    assert_eq!(checked.unanswerable_leads, BTreeMap::new());
    assert_eq!(checked.not_probed, Vec::<String>::new());
}
