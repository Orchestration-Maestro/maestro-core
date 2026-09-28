//! The ladder's counts per language: French and English questions counted
//! apart, and the counts `ask` gives absent when it did not run.

use super::ladder::{REFUSED, answered, ask_ending, ranked_at, right_section};
use crate::{
    eval::{LadderQuestion, LanguageCounts, score_ladder},
    suite::Suite,
};

/// Two answerable French questions and one English, then one unanswerable
/// question in each language.
fn mixed() -> Suite {
    [
        ("a0", "fr", true),
        ("a1", "fr", true),
        ("a2", "en", true),
        ("u0", "fr", false),
        ("u1", "en", false),
    ]
    .map(|(id, language, answerable)| {
        let expected = if answerable {
            serde_json::json!([{ "source_ref": "doc.md", "heading_path": [] }])
        } else {
            serde_json::json!([])
        };
        serde_json::json!({
            "schema": "maestro-suite/1", "id": id, "language": language,
            "question": format!("question {id}"), "answerable": answerable,
            "expected": expected,
        })
        .to_string()
    })
    .join("\n")
    .parse()
    .unwrap()
}

/// `a0` and `a2` ranked first and answered right, `a1` unranked and
/// refused, `u0` refused and `u1` answered.
fn rows() -> Vec<LadderQuestion> {
    let row = |id: &str, rank, ask| LadderQuestion {
        id: id.to_owned(),
        expected: if id.starts_with('a') {
            vec![right_section()]
        } else {
            Vec::new()
        },
        search: ranked_at(rank),
        ask,
    };
    vec![
        row("a0", Some(1), answered(vec![right_section()], 0)),
        row("a1", None, ask_ending(REFUSED)),
        row("a2", Some(1), answered(vec![right_section()], 0)),
        row("u0", None, ask_ending(REFUSED)),
        row("u1", None, answered(vec![right_section()], 0)),
    ]
}

#[test]
fn each_language_is_counted_apart_french_first() {
    let score = score_ladder(&mixed(), &rows());

    assert_eq!(
        score.languages,
        [
            LanguageCounts {
                language: "fr",
                answerable: 2,
                top_10: 1,
                unanswerable: 1,
                refused: Some(1),
                false_refusals: Some(1),
                supported_answers: Some(1),
            },
            LanguageCounts {
                language: "en",
                answerable: 1,
                top_10: 1,
                unanswerable: 1,
                refused: Some(0),
                false_refusals: Some(0),
                supported_answers: Some(1),
            },
        ]
    );
    assert!(score.to_markdown().ends_with(concat!(
        "| Language | Top-10 | Refused | False refusals | Supported answers |\n",
        "| --- | --- | --- | --- | --- |\n",
        "| fr | 1/2 | 1/1 | 1/2 | 1/2 |\n",
        "| en | 1/1 | 0/1 | 0/1 | 1/1 |\n",
    )));
}

#[test]
fn without_asks_the_counts_ask_gives_are_not_run() {
    let score = score_ladder(&mixed(), &rows()).without_asks();

    assert_eq!(score.languages[0].top_10, 1);
    assert!(score.languages.iter().all(|language| {
        language.refused.is_none()
            && language.false_refusals.is_none()
            && language.supported_answers.is_none()
    }));
    assert!(
        score
            .to_markdown()
            .contains("| fr | 1/2 | not run | not run | not run |\n")
    );
}
