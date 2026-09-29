//! The glossary expander: literal whole-word matching without case or
//! accents, bounded additions, and a strict, size-bounded table.

use crate::{
    query::understand,
    search::{
        Expansion, ExpansionFailure, Glossary, GlossaryError, MAX_GLOSSARY_BYTES, QueryExpander,
    },
};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};

/// A glossary of `entries`.
fn table(entries: &Value) -> String {
    json!({"schema": "maestro-glossary/1", "entries": entries}).to_string()
}

/// The glossary of the failure-count example.
fn failures() -> Glossary {
    Glossary::parse(
        table(&json!([
            {"id": "G1", "phrases": ["third failure", "troisième échec"],
             "add": ["NumberOfFailures", "Job's Number of Failures"]},
            {"id": "G2", "phrases": ["rerun"], "requires_any": ["job", "tâche", "plug-in"],
             "add": ["Rerunning a Job"]},
            {"id": "G3", "phrases": ["rerunning"], "add": ["Cyclic"]},
        ]))
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn a_glossary_keeps_the_digest_of_its_bytes() {
    let text = table(&json!([{"id": "G1", "phrases": ["x y"], "add": ["Z"]}]));
    let glossary = Glossary::parse(text.as_bytes()).unwrap();
    assert_eq!(glossary.digest(), &Digest::of(text.as_bytes()));
}

#[test]
fn a_phrase_matches_whole_words_without_case_or_accents() {
    let glossary = failures();
    let added = ["NumberOfFailures", "Job's Number of Failures"];
    assert_eq!(
        glossary.bridge("control-m job only after the Third Failure occurrence"),
        added
    );
    assert_eq!(glossary.bridge("seulement après le TROISIEME échec"), added);
    assert!(glossary.bridge("the third failures").is_empty());
    assert!(glossary.bridge("thethird failure").is_empty());
}

#[test]
fn context_words_gate_an_entry_and_its_output_is_never_matched_again() {
    let glossary = failures();
    assert!(glossary.bridge("rerun the report").is_empty());
    assert_eq!(glossary.bridge("rerun a job"), ["Rerunning a Job"]);
    assert_eq!(
        glossary.bridge("relancer (rerun) une tâche"),
        ["Rerunning a Job"]
    );
}

#[test]
fn a_term_is_added_once_and_never_when_the_question_holds_it() {
    let glossary = Glossary::parse(
        table(&json!([
            {"id": "A", "phrases": ["fails"], "add": ["Ended Not OK", "Rerun"]},
            {"id": "B", "phrases": ["failed"], "add": ["Rerun", "Output"]},
        ]))
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        glossary.bridge("it fails then failed"),
        ["Ended Not OK", "Rerun", "Output"]
    );
    assert_eq!(glossary.bridge("it fails and ended not ok"), ["Rerun"]);
}

#[test]
fn an_expansion_adds_at_most_eight_terms_and_512_bytes() {
    let many = (0..16)
        .map(|index| format!("Term{index}"))
        .collect::<Vec<_>>();
    let long = (0..16)
        .map(|index| format!("{index:02}{}", "x".repeat(100)))
        .collect::<Vec<_>>();
    let glossary = Glossary::parse(
        table(&json!([
            {"id": "many", "phrases": ["many"], "add": many},
            {"id": "long", "phrases": ["long"], "add": long},
        ]))
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(glossary.bridge("many"), &many[..8]);
    let added = glossary.bridge("long");
    assert_eq!(added, &long[..4]);
    assert!(added.join(" ").len() <= 512);
}

#[tokio::test]
async fn the_expander_appends_its_terms_or_finds_no_match() {
    let glossary = failures();
    let question = understand("notify after the third failure");
    assert_eq!(
        glossary.expand(&question).await,
        Ok(Expansion {
            passage: "notify after the third failure NumberOfFailures Job's Number of Failures"
                .to_owned(),
            keywords: "NumberOfFailures Job's Number of Failures".to_owned(),
        })
    );
    assert_eq!(
        glossary.expand(&understand("kill a job")).await,
        Err(ExpansionFailure::NoMatch)
    );
    assert_eq!(ExpansionFailure::NoMatch.code(), "intent_no_match");
}

#[test]
fn a_glossary_is_strict_json_of_its_contract() {
    let entry = json!({"id": "G1", "phrases": ["x"], "add": ["Y"]});
    for (text, expected) in [
        (
            json!({"schema": "maestro-glossary/2", "entries": []}).to_string(),
            "Schema",
        ),
        (
            json!({"schema": "maestro-glossary/1", "entries": [], "extra": 1}).to_string(),
            "Json",
        ),
        (
            table(&json!([{"id": "G1", "phrases": ["x"], "add": ["Y"], "note": ""}])),
            "Json",
        ),
        (table(&json!([["G1", ["x"], [], ["Y"]]])), "Json"),
        (format!("{} {{}}", table(&json!([entry]))), "Json"),
        (table(&json!([entry, entry])), "DuplicateId"),
    ] {
        let error = Glossary::parse(text.as_bytes()).unwrap_err();
        assert!(
            format!("{error:?}").starts_with(expected),
            "{text}: {error:?}"
        );
    }
    assert!(matches!(
        Glossary::parse(&[0xff, 0xfe]),
        Err(GlossaryError::Encoding)
    ));
}

#[test]
fn a_glossary_keeps_its_bounds() {
    let refused = |entry: Value| Glossary::parse(table(&json!([entry])).as_bytes()).unwrap_err();
    let seventeen = (0..17).map(|index| format!("w{index}")).collect::<Vec<_>>();
    for entry in [
        json!({"id": "G", "phrases": [], "add": ["Y"]}),
        json!({"id": "G", "phrases": ["x"], "add": []}),
        json!({"id": "G", "phrases": seventeen, "add": ["Y"]}),
        json!({"id": "G", "phrases": ["x"], "add": seventeen}),
        json!({"id": "G", "phrases": ["x"], "requires_any": seventeen, "add": ["Y"]}),
    ] {
        assert!(matches!(refused(entry), GlossaryError::Bounds));
    }
    for entry in [
        json!({"id": " ", "phrases": ["x"], "add": ["Y"]}),
        json!({"id": "G", "phrases": ["x\ny"], "add": ["Y"]}),
        json!({"id": "G", "phrases": ["x"], "add": ["y".repeat(129)]}),
    ] {
        assert!(matches!(refused(entry), GlossaryError::BlankText));
    }
    for entry in [
        json!({"id": "G", "phrases": ["--"], "add": ["Y"]}),
        json!({"id": "G", "phrases": ["x"], "requires_any": ["?"], "add": ["Y"]}),
    ] {
        assert!(matches!(refused(entry), GlossaryError::NoWords));
    }
    let entries = (0..1025)
        .map(|index| {
            json!({"id": format!("G{index}"), "phrases": [format!("p{index}")], "add": ["Y"]})
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        Glossary::parse(table(&json!(entries)).as_bytes()),
        Err(GlossaryError::Bounds)
    ));
    let padded = format!("{}{}", table(&json!([])), " ".repeat(MAX_GLOSSARY_BYTES));
    assert!(matches!(
        Glossary::parse(padded.as_bytes()),
        Err(GlossaryError::TooLarge)
    ));
}

#[test]
fn two_entries_never_give_the_same_phrase() {
    let text = table(&json!([
        {"id": "A", "phrases": ["Third Failure"], "add": ["Y"]},
        {"id": "B", "phrases": ["third  failure"], "add": ["Z"]},
    ]));
    assert!(matches!(
        Glossary::parse(text.as_bytes()),
        Err(GlossaryError::AmbiguousPhrase(phrase)) if phrase == "third failure"
    ));
}
