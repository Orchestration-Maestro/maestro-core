//! Resolving an expected section in its canonicalized document: a heading path
//! that one section has names that section, and an occurrence counts the
//! sections of a repeated path from 1 in the document's order; a path no
//! section has, a repeated path without an occurrence, and an occurrence on a
//! path that does not repeat, or past its last repeat, are refused.
#![cfg(test)]

use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_knowledge::suite::{ExpectedSection, Suite, Unresolved};
use serde_json::json;
use std::{error, num::NonZeroU32};

/// A document whose sections, in order, have the heading paths:
/// 0 `Rotation`; 1 `Rotation › By size`; 2 `Rotation › By size › Note`;
/// 3 `Rotation › Example`; 4 `Rotation › By time`;
/// 5 `Rotation › By time › Note`; 6 `Rotation › Example`.
const MARKDOWN: &str = "# Rotation\n\nLogs rotate by size or by time.\n\n\
    ## By size\n\nAt 100 MiB.\n\n### Note\n\nCompressed files count too.\n\n\
    ## Example\n\nRotate at 50 MiB.\n\n## By time\n\nEvery day at midnight.\n\n\
    ### Note\n\nTimes are in UTC.\n\n## Example\n\nRotate every day.\n";

fn document() -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(MARKDOWN, "rotation.md")).unwrap()
}

/// The section a suite names by `heading_path` and `occurrence`, read through
/// the suite's contract.
fn named(heading_path: &[&str], occurrence: Option<u32>) -> ExpectedSection {
    let mut section = json!({
        "source_ref": "corpus-path:rotation.md",
        "heading_path": heading_path
    });
    if let Some(occurrence) = occurrence {
        section["occurrence"] = json!(occurrence);
    }
    let line = json!({
        "schema": "maestro-suite/1",
        "id": "rotation",
        "language": "en",
        "question": "When do logs rotate?",
        "answerable": true,
        "expected": [section]
    });
    let suite: Suite = line.to_string().parse().unwrap();
    suite.questions[0].expected[0].clone()
}

#[test]
fn a_heading_path_that_one_section_has_names_that_section() {
    let document = document();
    for (heading_path, index) in [
        (&["Rotation"][..], 0),
        (&["Rotation", "By size"][..], 1),
        // The title repeats under another parent, the path does not.
        (&["Rotation", "By size", "Note"][..], 2),
        (&["Rotation", "By time", "Note"][..], 5),
    ] {
        let resolved = named(heading_path, None).resolve(&document);
        assert_eq!(resolved, Ok(&document.sections[index]), "{heading_path:?}");
    }
}

#[test]
fn an_occurrence_counts_the_sections_of_a_repeated_path_from_1_in_document_order() {
    let document = document();
    for (occurrence, index) in [(1, 3), (2, 6)] {
        let resolved = named(&["Rotation", "Example"], Some(occurrence)).resolve(&document);
        assert_eq!(resolved, Ok(&document.sections[index]), "{occurrence}");
    }
}

#[test]
fn a_heading_path_that_no_section_has_is_refused() {
    let document = document();
    for (heading_path, occurrence) in [
        (&["Rotation", "By volume"][..], None),
        (&["Rotation", "Note"][..], None),
        (&["Note"][..], None),
        (&["Rotation", "by size"][..], None),
        (&["Rotation", "By size", "Note", "Sizes"][..], None),
        (&[][..], None),
        (&["Rotation", "By volume"][..], Some(2)),
    ] {
        let resolved = named(heading_path, occurrence).resolve(&document);
        assert_eq!(resolved, Err(Unresolved::NoSection), "{heading_path:?}");
    }
}

#[test]
fn a_repeated_heading_path_without_an_occurrence_is_refused() {
    let document = document();
    let resolved = named(&["Rotation", "Example"], None).resolve(&document);
    assert_eq!(resolved, Err(Unresolved::Ambiguous { sections: 2 }));
}

#[test]
fn an_occurrence_on_a_heading_path_that_does_not_repeat_is_refused() {
    let document = document();
    for (heading_path, occurrence) in [
        (&["Rotation", "By size"][..], 1),
        (&["Rotation", "By size"][..], 2),
        (&["Rotation", "By time", "Note"][..], 1),
    ] {
        let resolved = named(heading_path, Some(occurrence)).resolve(&document);
        assert_eq!(
            resolved,
            Err(Unresolved::NotRepeated),
            "{heading_path:?} {occurrence}"
        );
    }
}

#[test]
fn an_occurrence_past_the_last_repeat_is_refused() {
    let document = document();
    let resolved = named(&["Rotation", "Example"], Some(3)).resolve(&document);
    let occurrence = NonZeroU32::new(3).unwrap();
    assert_eq!(
        resolved,
        Err(Unresolved::PastLastRepeat {
            occurrence,
            sections: 2
        })
    );
}

#[test]
fn a_refusal_says_why_the_name_gives_no_one_section() {
    let occurrence = NonZeroU32::new(3).unwrap();
    for (refusal, says) in [
        (Unresolved::NoSection, "no section"),
        (Unresolved::Ambiguous { sections: 2 }, "2 sections"),
        (Unresolved::NotRepeated, "takes no occurrence"),
        (
            Unresolved::PastLastRepeat {
                occurrence,
                sections: 2,
            },
            "occurrence 3 is past the 2 sections",
        ),
    ] {
        let shown = refusal.to_string();
        assert!(
            shown.contains(says) && shown.contains("heading path"),
            "{shown}"
        );
        assert!(error::Error::source(&refusal).is_none());
    }
}
