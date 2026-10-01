//! The check on the public synthetic collection: every section its suite
//! `synthetic` expects is one section of its corpus.

use super::check::{Counts, check};
use std::{collections::BTreeMap, path::Path};

#[test]
fn every_name_of_the_synthetic_suite_gives_one_section() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    let fixture = workspace.join("tests").join("fixtures").join("synthetic");
    let manifest = fixture.join("corpus").join("maestro-corpus.jsonl");
    let checked = check(&fixture.join("evals"), &manifest);
    assert_eq!(checked.problems, Vec::<String>::new());
    assert!(checked.changed.is_empty(), "{:?}", checked.changed);
    let counts = Counts {
        questions: 56,
        unanswerable: 8,
        sections: 61,
        documents: 0,
    };
    assert_eq!(
        checked.suites,
        BTreeMap::from([("synthetic".to_owned(), counts)])
    );
}
