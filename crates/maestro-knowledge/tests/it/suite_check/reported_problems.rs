//! What the check reports as problems: a name that gives no one section nor
//! a document without sections, a `source_ref` that no manifest line
//! declares, or that several declare with different bytes, a question that
//! names one section or document twice, a document that cannot be read, a
//! manifest line that is not a corpus entry, a file that is not a suite and a
//! directory without one; and, without a problem, a document whose file
//! changed since its manifest line, a `source_ref` that several lines declare
//! with the same bytes, and two documents without sections named by one
//! question.

use super::{
    check::Counts,
    scratch::{
        LOGS_REF, NOTES, NOTES_REF, ROTATION, ROTATION_REF, Scratch, corpus, line, name, question,
    },
};
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn a_name_that_gives_no_one_section_nor_a_document_is_a_problem_of_its_question() {
    let scratch = corpus();
    scratch.suite(
        "names",
        &[
            question(
                "found",
                &json!([name(ROTATION_REF, &["Rotation", "By size"])]),
            ),
            question("whole", &json!([name(NOTES_REF, &[])])),
            question(
                "nowhere",
                &json!([name(ROTATION_REF, &["Rotation", "Size"])]),
            ),
            question(
                "which",
                &json!([name(ROTATION_REF, &["Rotation", "Example"])]),
            ),
            question("sectioned", &json!([name(ROTATION_REF, &[])])),
            question("headed", &json!([name(NOTES_REF, &["Cause"])])),
            question("unknown", &json!([])),
        ],
    );
    let checked = scratch.check();
    assert_eq!(
        checked.problems,
        [
            format!("names: headed: {NOTES_REF} [\"Cause\"]: no section has this heading path"),
            format!(
                "names: nowhere: {ROTATION_REF} [\"Rotation\", \"Size\"]: no section has this \
                 heading path"
            ),
            format!(
                "names: sectioned: {ROTATION_REF} []: an empty heading path names a document \
                 without sections, but this document has 5 sections"
            ),
            format!(
                "names: which: {ROTATION_REF} [\"Rotation\", \"Example\"]: 2 sections have this \
                 heading path, and no occurrence says which"
            ),
        ]
    );
    let counts = Counts {
        questions: 7,
        unanswerable: 1,
        sections: 1,
        documents: 1,
    };
    assert_eq!(checked.suites["names"], counts);
    assert!(checked.changed.is_empty(), "{:?}", checked.changed);
}

#[test]
fn a_source_ref_no_line_declares_or_lines_declare_with_different_bytes_is_a_problem() {
    let scratch = corpus();
    let edited = format!("{ROTATION}\nEdited.\n");
    scratch.document("copy.md", edited.as_bytes());
    scratch.document("notes-copy.md", NOTES.as_bytes());
    scratch.manifest(&[
        line("rotation.md", ROTATION.as_bytes(), ROTATION_REF),
        line("copy.md", edited.as_bytes(), ROTATION_REF),
        line("notes.md", NOTES.as_bytes(), NOTES_REF),
        line("notes-copy.md", NOTES.as_bytes(), NOTES_REF),
    ]);
    let lost = "https://handbook.example.org/lost";
    scratch.suite(
        "refs",
        &[
            question("copied", &json!([name(ROTATION_REF, &["Rotation"])])),
            question("same-bytes", &json!([name(NOTES_REF, &[])])),
            question("lost", &json!([name(lost, &["Lost"])])),
        ],
    );
    let checked = scratch.check();
    assert_eq!(
        checked.problems,
        [
            format!(
                "refs: copied: {ROTATION_REF} is declared by 2 manifest lines with different \
                 bytes, which the importer holds"
            ),
            format!("refs: lost: no manifest line declares {lost}"),
        ]
    );
    assert_eq!(checked.suites["refs"].documents, 1);
}

#[test]
fn a_question_that_names_one_section_or_document_twice_is_a_problem() {
    let scratch = corpus();
    let by_size = name(ROTATION_REF, &["Rotation", "By size"]);
    let notes = name(NOTES_REF, &[]);
    let logs = name(LOGS_REF, &[]);
    scratch.suite(
        "twice",
        &[
            question("section", &json!([by_size, by_size])),
            question("document", &json!([notes, notes])),
            question("both", &json!([by_size, notes])),
            question("two-documents", &json!([notes, logs])),
        ],
    );
    let checked = scratch.check();
    assert_eq!(
        checked.problems,
        [
            format!("twice: document: names {NOTES_REF} [] twice"),
            format!("twice: section: names {ROTATION_REF} [\"Rotation\", \"By size\"] twice"),
        ]
    );
    let counts = Counts {
        questions: 4,
        unanswerable: 0,
        sections: 3,
        documents: 5,
    };
    assert_eq!(checked.suites["twice"], counts);
}

#[test]
fn a_document_that_cannot_be_read_is_a_problem_of_each_question_that_names_it() {
    let scratch = Scratch::new();
    let absent = "https://handbook.example.org/absent";
    let garbled = "https://handbook.example.org/garbled";
    let garbled_bytes = b"# Rotation \xff\n\nNot UTF-8.\n";
    scratch.document("garbled.md", garbled_bytes);
    scratch.manifest(&[
        line("absent.md", b"# Absent\n", absent),
        line("garbled.md", garbled_bytes, garbled),
    ]);
    scratch.suite(
        "files",
        &[
            question("absent", &json!([name(absent, &["Absent"])])),
            question("again", &json!([name(absent, &["Absent"])])),
            question("garbled", &json!([name(garbled, &["Rotation"])])),
        ],
    );
    let problems = scratch.check().problems;
    let [first, second, third] = problems.as_slice() else {
        panic!("three problems: {problems:?}");
    };
    assert!(
        first.starts_with("files: absent: absent.md cannot be read: "),
        "{first}"
    );
    assert!(
        second.starts_with("files: again: absent.md cannot be read: "),
        "{second}"
    );
    assert_eq!(third, "files: garbled: garbled.md is not UTF-8");
}

#[test]
fn a_manifest_line_that_is_not_a_corpus_entry_is_a_problem() {
    let scratch = corpus();
    let mut lines = vec![line("rotation.md", ROTATION.as_bytes(), ROTATION_REF)];
    lines.push(json!({"schema": "maestro-corpus/1", "path": "notes.md"}));
    scratch.manifest(&lines);
    scratch.suite(
        "entries",
        &[question(
            "found",
            &json!([name(ROTATION_REF, &["Rotation"])]),
        )],
    );
    let problems = scratch.check().problems;
    let [problem] = problems.as_slice() else {
        panic!("one problem: {problems:?}");
    };
    assert!(problem.starts_with("manifest line 2: "), "{problem}");
}

#[test]
fn a_file_that_is_not_a_suite_is_a_problem_and_a_file_of_another_kind_is_no_suite() {
    let scratch = corpus();
    let by_size = name(ROTATION_REF, &["Rotation", "By size"]);
    scratch.suite("good", &[question("found", &json!([by_size]))]);
    let mut unanswerable = question("claimed", &json!([by_size]));
    unanswerable["answerable"] = json!(false);
    scratch.suite("broken", &[unanswerable]);
    scratch.evals_file("README.md", "Not a suite.\n");
    let checked = scratch.check();
    assert_eq!(
        checked.problems,
        [
            "suite broken: line 1 says answerable is false, but a question is answerable \
             exactly when it expects a section"
        ]
    );
    let names: Vec<&str> = checked.suites.keys().map(String::as_str).collect();
    assert_eq!(names, ["good"]);
}

#[test]
fn a_directory_without_a_suite_is_a_problem() {
    let scratch = corpus();
    scratch.evals_file("README.md", "Not a suite.\n");
    let problems = scratch.check().problems;
    let [problem] = problems.as_slice() else {
        panic!("one problem: {problems:?}");
    };
    assert!(problem.ends_with(" holds no suite"), "{problem}");
}

#[test]
fn a_document_whose_file_changed_since_its_line_is_checked_and_listed() {
    let scratch = Scratch::new();
    scratch.document("rotation.md", ROTATION.as_bytes());
    scratch.manifest(&[line("rotation.md", b"# Rotation\n\nOlder.\n", ROTATION_REF)]);
    scratch.suite(
        "changed",
        &[question(
            "found",
            &json!([name(ROTATION_REF, &["Rotation", "By time"])]),
        )],
    );
    let checked = scratch.check();
    assert_eq!(checked.problems, Vec::<String>::new());
    assert_eq!(checked.changed, BTreeSet::from([ROTATION_REF.to_owned()]));
    assert_eq!(checked.suites["changed"].sections, 1);
}
