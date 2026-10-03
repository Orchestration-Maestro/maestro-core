//! What the graph evaluation tests share: a synthetic source, a suite of its
//! questions, labels anchored in it, and the checker run over them.

use crate::{
    eval::graph::{CheckError, CheckedLabels, Located, Original, Stage, check_labels},
    suite::Suite,
};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::convert::Infallible;

/// The synthetic source every anchor quotes.
pub(super) const SOURCE: &str = "# Lantern\n\n| Parameter | Default |\n| --- | --- |\n\
     | retries | 3 |\n| label | café |\n\nThe relay requires the lantern.\n\
     The lantern is part of the beacon.\n";
/// Its `source_ref`.
pub(super) const SOURCE_REF: &str = "graph/lantern.md";
/// The revision the checker's source lookup gives for it.
pub(super) const REVISION: &str = "revision-1";

/// The span of `quote` in [`SOURCE`].
pub(super) fn span_of(quote: &str) -> [usize; 2] {
    let start = SOURCE.find(quote).unwrap();
    [start, start + quote.len()]
}

/// An anchor quoting `quote` of [`SOURCE`] exactly.
pub(super) fn anchor(quote: &str) -> Value {
    json!({
        "source_ref": SOURCE_REF,
        "original": Digest::of(SOURCE.as_bytes()).as_str(),
        "span": span_of(quote),
        "quote": Digest::of(quote.as_bytes()).as_str(),
    })
}

/// A link from `subject` to `object` by `predicate`, anchored in `quotes`.
pub(super) fn link(subject: &str, predicate: &str, object: &str, quotes: &[&str]) -> Value {
    let anchors: Vec<Value> = quotes.iter().map(|quote| anchor(quote)).collect();
    json!({
        "subject": subject,
        "predicate": predicate,
        "object": object,
        "conditions": [],
        "anchors": anchors,
    })
}

/// An accepted independent review.
pub(super) fn accepted() -> Value {
    json!({"reviewer_card": Digest::of(b"reviewer").as_str(), "disposition": "accepted"})
}

/// An answerable label of `id` in `family` of `kind`, with `proofs`, each a
/// list of links, reviewed and accepted.
pub(super) fn answerable(id: &str, family: &str, kind: &str, proofs: &[Vec<Value>]) -> Value {
    let proofs: Vec<Value> = proofs.iter().map(|links| json!({"links": links})).collect();
    json!({
        "schema": "maestro-graph-labels/1",
        "id": id,
        "family": family,
        "kind": kind,
        "language": "en",
        "proofs": proofs,
        "review": accepted(),
    })
}

/// A reviewed unanswerable label of `id` in `family`.
pub(super) fn unanswerable(id: &str, family: &str) -> Value {
    json!({
        "schema": "maestro-graph-labels/1",
        "id": id,
        "family": family,
        "kind": "unanswerable",
        "language": "fr",
        "proofs": [],
        "unanswerable_reason": "no source names a successor",
        "review": accepted(),
    })
}

/// The retries default: one relationship link.
pub(super) fn retries() -> Value {
    link("retries", "DEFAULTS_TO", "3", &["| retries | 3 |\n"])
}

/// A two-link chain: relay requires lantern, lantern part of beacon.
pub(super) fn chain() -> Vec<Value> {
    vec![
        link(
            "relay",
            "REQUIRES",
            "lantern",
            &["The relay requires the lantern."],
        ),
        link(
            "lantern",
            "PART_OF",
            "beacon",
            &["The lantern is part of the beacon."],
        ),
    ]
}

/// The three labels of [`suite`]: a relationship, a multi-hop chain and an
/// unanswerable question.
pub(super) fn labels() -> Vec<Value> {
    vec![
        answerable("q-1", "f-1", "relationship", &[vec![retries()]]),
        answerable("q-2", "f-2", "multi_hop", &[chain()]),
        unanswerable("q-3", "f-3"),
    ]
}

/// The suite whose questions [`labels`] label: `q-1` and `q-2` answerable,
/// `q-3` not, `q-3` in French.
pub(super) fn suite() -> Suite {
    let expected = json!([{"source_ref": SOURCE_REF, "heading_path": ["Lantern"]}]);
    let lines = [
        json!({"schema": "maestro-suite/1", "id": "q-1", "language": "en",
            "question": "What is the retries default?", "answerable": true,
            "expected": expected}),
        json!({"schema": "maestro-suite/1", "id": "q-2", "language": "en",
            "question": "What does the relay need?", "answerable": true,
            "expected": expected}),
        json!({"schema": "maestro-suite/1", "id": "q-3", "language": "fr",
            "question": "Quel est le successeur ?", "answerable": false,
            "expected": []}),
    ];
    let text: Vec<String> = lines.iter().map(Value::to_string).collect();
    text.join("\n").parse().unwrap()
}

/// `labels` as the JSON Lines text of a labels file.
pub(super) fn text(labels: &[Value]) -> String {
    let lines: Vec<String> = labels.iter().map(Value::to_string).collect();
    lines.join("\n")
}

/// The source lookup of the tests: [`SOURCE`] as [`REVISION`] for its
/// `source_ref` and digest, nothing for any other.
pub(super) fn source(source_ref: &str, original: &Digest) -> Option<Original> {
    (source_ref == SOURCE_REF && *original == Digest::of(SOURCE.as_bytes())).then(|| Original {
        revision_id: REVISION.to_owned(),
        bytes: SOURCE.as_bytes().to_vec(),
    })
}

/// Checks `labels` against [`suite`] at `stage`, with their own digest.
pub(super) fn check(
    labels: &[Value],
    stage: Stage,
) -> Result<CheckedLabels, CheckError<Infallible>> {
    let text = text(labels);
    check_labels(
        &suite(),
        &text,
        &Digest::of(text.as_bytes()),
        stage,
        |reference, digest| Ok(source(reference, digest)),
    )
}

/// The located anchor of `quote` in [`REVISION`].
pub(super) fn located(quote: &str) -> Located {
    Located {
        revision_id: REVISION.to_owned(),
        span: span_of(quote),
    }
}
