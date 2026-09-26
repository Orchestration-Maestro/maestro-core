//! `maestro-suite/1`: a suite, one JSON line per question, parses into typed
//! questions in the order of their lines, an expected section with an empty
//! heading path among them; an unknown or repeated key at any depth, a line
//! or an expected section written as an array, a named value written as an
//! object, a value the contract does not name, an occurrence below 1, an
//! `answerable` that disagrees with `expected`, an id given twice and a line
//! that is not one JSON object are refused, each naming its line, and so is a
//! text without a question.
#![cfg(test)]

use maestro_knowledge::suite::{Error, Language, Schema, Suite};
use serde_json::{Value, json};
use std::{error, fmt::Write as _, num::NonZeroU32};

/// A question no test changes, for the first line of a suite whose second
/// line is under test.
fn opening() -> Value {
    json!({
        "schema": "maestro-suite/1",
        "id": "compost-turning",
        "language": "en",
        "question": "How often should compost be turned?",
        "answerable": true,
        "expected": [{
            "source_ref": "https://example.org/garden/compost",
            "heading_path": ["Compost", "Turning"]
        }]
    })
}

/// An answerable question with two expected sections, the second under a
/// heading path its document repeats.
fn answerable() -> Value {
    json!({
        "schema": "maestro-suite/1",
        "id": "raised-bed-depth",
        "language": "en",
        "question": "How deep should a raised bed be?",
        "answerable": true,
        "expected": [
            {
                "source_ref": "https://example.org/garden/raised-beds",
                "heading_path": ["Building raised beds", "Depth"]
            },
            {
                "source_ref": "corpus-path:beds/soil.md",
                "heading_path": ["Soil", "Example"],
                "occurrence": 2
            }
        ]
    })
}

/// A question no section answers.
fn unanswerable() -> Value {
    json!({
        "schema": "maestro-suite/1",
        "id": "greenhouse-heating",
        "language": "fr",
        "question": "Comment chauffer une serre en hiver ?",
        "answerable": false,
        "expected": []
    })
}

/// A suite text: each of `lines` on its own line.
fn suite(lines: &[Value]) -> String {
    let mut text = String::new();
    for line in lines {
        writeln!(text, "{line}").unwrap();
    }
    text
}

fn parse(text: &str) -> Result<Suite, Error> {
    text.parse()
}

/// The refusal, as displayed, of the suite whose first line is [`opening`]
/// and whose second is `line`, which must be refused as JSON the contract
/// does not accept, on line 2.
fn line_2_refusal(line: &str) -> String {
    let text = format!("{}\n{line}\n", opening());
    match parse(&text) {
        Err(refusal @ Error::Json { line: 2, .. }) => refusal.to_string(),
        other => panic!("expected a JSON refusal of line 2 in {text}, got {other:?}"),
    }
}

#[test]
fn an_answerable_question_parses_into_typed_values() {
    let parsed = parse(&suite(&[answerable()])).unwrap();
    let [question] = parsed.questions.as_slice() else {
        panic!("one question: {:?}", parsed.questions);
    };
    assert_eq!(question.schema, Schema::V1);
    assert_eq!(question.id, "raised-bed-depth");
    assert_eq!(question.language, Language::En);
    assert_eq!(question.question, "How deep should a raised bed be?");
    assert!(question.answerable);
    let [depth, example] = question.expected.as_slice() else {
        panic!("two expected sections: {:?}", question.expected);
    };
    assert_eq!(depth.source_ref, "https://example.org/garden/raised-beds");
    assert_eq!(depth.heading_path, ["Building raised beds", "Depth"]);
    assert_eq!(depth.occurrence, None);
    assert_eq!(example.source_ref, "corpus-path:beds/soil.md");
    assert_eq!(example.heading_path, ["Soil", "Example"]);
    assert_eq!(example.occurrence.map(NonZeroU32::get), Some(2));
}

#[test]
fn an_empty_heading_path_names_a_document_without_sections() {
    let mut line = answerable();
    line["expected"] = json!([{"source_ref": "corpus-path:notes/frost.md", "heading_path": []}]);
    let parsed = parse(&suite(&[line])).unwrap();
    let [whole] = parsed.questions[0].expected.as_slice() else {
        panic!("one expected section: {:?}", parsed.questions[0].expected);
    };
    assert_eq!(whole.source_ref, "corpus-path:notes/frost.md");
    assert!(whole.heading_path.is_empty(), "{:?}", whole.heading_path);
    assert_eq!(whole.occurrence, None);
}

#[test]
fn a_suite_holds_its_questions_in_the_order_of_their_lines() {
    let parsed = parse(&suite(&[answerable(), unanswerable()])).unwrap();
    let [first, second] = parsed.questions.as_slice() else {
        panic!("two questions: {:?}", parsed.questions);
    };
    assert_eq!(first.id, "raised-bed-depth");
    assert_eq!(second.schema, Schema::V1);
    assert_eq!(second.id, "greenhouse-heating");
    assert_eq!(second.language, Language::Fr);
    assert_eq!(second.question, "Comment chauffer une serre en hiver ?");
    assert!(!second.answerable);
    assert!(second.expected.is_empty(), "{:?}", second.expected);
}

#[test]
fn a_value_the_contract_does_not_name_is_refused() {
    for (key, value) in [
        ("schema", "maestro-suite/2"),
        ("schema", "maestro-corpus/1"),
        ("language", "de"),
        ("language", "FR"),
        ("language", "fr-CA"),
        ("language", ""),
    ] {
        let mut question = answerable();
        question[key] = json!(value);
        let reason = line_2_refusal(&question.to_string());
        assert!(
            reason.contains("unknown variant"),
            "{key} = {value}: {reason}"
        );
    }
}

#[test]
fn a_named_value_written_as_an_object_is_refused() {
    for (key, value) in [("schema", "maestro-suite/1"), ("language", "en")] {
        let mut question = answerable();
        question[key] = json!({ value: null });
        let reason = line_2_refusal(&question.to_string());
        assert!(reason.contains("invalid type: map"), "{key}: {reason}");
    }
}

#[test]
fn an_object_written_as_an_array_is_refused_at_every_level() {
    let question = answerable();
    // Each object's values in the order its fields are declared, without keys.
    let whole = json!([
        "maestro-suite/1",
        "raised-bed-depth",
        "en",
        "How deep should a raised bed be?",
        true,
        question["expected"]
    ]);
    let section = json!([
        "https://example.org/garden/raised-beds",
        ["Building raised beds", "Depth"],
        null
    ]);
    for (pointer, values) in [("", whole), ("/expected/0", section)] {
        let mut changed = answerable();
        *changed.pointer_mut(pointer).unwrap() = values;
        let reason = line_2_refusal(&changed.to_string());
        assert!(
            reason.contains("invalid type: sequence, expected a JSON object"),
            "{pointer}: {reason}"
        );
    }
}

#[test]
fn an_unknown_key_is_refused_at_every_level() {
    for pointer in ["", "/expected/0", "/expected/1"] {
        let mut question = answerable();
        question.pointer_mut(pointer).unwrap()["season"] = json!("spring");
        let reason = line_2_refusal(&question.to_string());
        assert!(
            reason.contains("unknown field `season`"),
            "{pointer}: {reason}"
        );
    }
}

#[test]
fn a_key_given_twice_is_refused_at_every_level() {
    let text = answerable().to_string();
    for (once, again) in [
        ("\"id\":\"raised-bed-depth\"", "\"id\":\"raised-bed-width\""),
        ("\"answerable\":true", "\"answerable\":true"),
        ("\"occurrence\":2", "\"occurrence\":3"),
        (
            "\"source_ref\":\"corpus-path:beds/soil.md\"",
            "\"source_ref\":\"corpus-path:beds/loam.md\"",
        ),
    ] {
        assert!(text.contains(once), "{once} is not in {text}");
        let twice = text.replacen(once, &format!("{once},{again}"), 1);
        let reason = line_2_refusal(&twice);
        assert!(reason.contains("duplicate field"), "{again}: {reason}");
    }
}

#[test]
fn a_missing_key_is_refused_at_every_level() {
    for (pointer, key) in [
        ("", "schema"),
        ("", "id"),
        ("", "language"),
        ("", "question"),
        ("", "answerable"),
        ("", "expected"),
        ("/expected/0", "source_ref"),
        ("/expected/0", "heading_path"),
    ] {
        let mut question = answerable();
        let object = question.pointer_mut(pointer).unwrap();
        object.as_object_mut().unwrap().remove(key).unwrap();
        let reason = line_2_refusal(&question.to_string());
        assert!(
            reason.contains(&format!("missing field `{key}`")),
            "{pointer}/{key}: {reason}"
        );
    }
}

#[test]
fn an_occurrence_is_a_whole_number_from_1() {
    for occurrence in [json!(0), json!(-1), json!(1.5), json!("2"), json!([2])] {
        let mut question = answerable();
        question["expected"][1]["occurrence"] = occurrence;
        line_2_refusal(&question.to_string());
    }
    let mut first = answerable();
    first["expected"][1]["occurrence"] = json!(1);
    let parsed = parse(&suite(&[first])).unwrap();
    let occurrence = parsed.questions[0].expected[1].occurrence;
    assert_eq!(occurrence.map(NonZeroU32::get), Some(1));
}

#[test]
fn answerable_is_false_exactly_when_expected_is_empty() {
    let mut without_sections = answerable();
    without_sections["expected"] = json!([]);
    let mut with_sections = unanswerable();
    with_sections["expected"] = answerable()["expected"].clone();
    for (line, said) in [(without_sections, true), (with_sections, false)] {
        let refusal = parse(&suite(&[opening(), line])).unwrap_err();
        assert!(
            matches!(refusal, Error::Answerable { line: 2, answerable } if answerable == said),
            "{refusal:?}"
        );
        let shown = refusal.to_string();
        assert!(
            shown.contains("line 2") && shown.contains("answerable"),
            "{shown}"
        );
        assert!(error::Error::source(&refusal).is_none());
    }
}

#[test]
fn an_id_given_twice_is_refused_naming_both_lines() {
    let mut again = unanswerable();
    again["question"] = json!("Comment chauffer une serre sans électricité ?");
    let refusal = parse(&suite(&[unanswerable(), answerable(), again])).unwrap_err();
    assert!(
        matches!(
            &refusal,
            Error::DuplicateId { line: 3, first: 1, id } if id == "greenhouse-heating"
        ),
        "{refusal:?}"
    );
    let shown = refusal.to_string();
    assert!(
        shown.contains("line 3")
            && shown.contains("line 1")
            && shown.contains("`greenhouse-heating`"),
        "{shown}"
    );
    assert!(error::Error::source(&refusal).is_none());
}

#[test]
fn a_line_that_is_not_one_json_object_is_refused_with_its_number() {
    let line = answerable().to_string();
    let pretty = serde_json::to_string_pretty(&answerable()).unwrap();
    for (text, number) in [
        (format!("{}\n\n{line}\n", opening()), 2),
        (format!("{line} {line}\n"), 1),
        (format!("{pretty}\n"), 1),
        (format!("[{line}]\n"), 1),
        ("\"raised-bed-depth\"\n".to_owned(), 1),
        (format!("{}\n{}\n", opening(), line.replacen('}', "", 1)), 2),
    ] {
        match parse(&text) {
            Err(Error::Json { line, .. }) => assert_eq!(line, number, "{text}"),
            other => panic!("expected a JSON refusal of {text}, got {other:?}"),
        }
    }
}

#[test]
fn a_text_without_a_question_is_refused() {
    let refusal = parse("").unwrap_err();
    assert!(matches!(refusal, Error::Empty), "{refusal:?}");
    let shown = refusal.to_string();
    assert!(
        shown.contains("maestro-suite/1") && shown.contains("no question"),
        "{shown}"
    );
    assert!(error::Error::source(&refusal).is_none());
}

#[test]
fn a_refusal_names_the_contract_and_its_line_and_keeps_its_cause() {
    let refusal = parse(&format!("{}\n{{}}\n", opening())).unwrap_err();
    let shown = refusal.to_string();
    assert!(
        shown.contains("maestro-suite/1") && shown.contains("line 2"),
        "{shown}"
    );
    assert!(error::Error::source(&refusal).is_some());
}
