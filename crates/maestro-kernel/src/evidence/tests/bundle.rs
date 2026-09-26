//! Bundles: `maestro-evidence/1` as JSON, its evidence apart from its trace,
//! strict when read, and equal once written and read back.

use super::support::{SOURCE_REF, bundle, span_of};
use crate::{
    artifact::Digest,
    evidence::{Bundle, RouteStatus, Span},
};
use serde_json::{Value, json};

/// The bundle the JSON `value` holds, or why it is refused.
fn read(value: &Value) -> Result<Bundle, String> {
    serde_json::from_str(&value.to_string()).map_err(|error| error.to_string())
}

/// The JSON `bundle` writes, or why it is refused: a refused bundle writes
/// nothing at all.
fn write(bundle: &Bundle) -> Result<String, String> {
    let mut written = Vec::new();
    let outcome = serde_json::to_writer(&mut written, bundle);
    let text = String::from_utf8(written).unwrap();
    match outcome {
        Ok(()) => Ok(text),
        Err(error) => {
            assert_eq!(text, "", "a refused bundle writes nothing");
            Err(error.to_string())
        }
    }
}

/// `text` written as a bundle writes a digest.
fn digest_of(text: &str) -> String {
    format!("sha256:{}", Digest::of(text.as_bytes()).as_str())
}

#[test]
fn a_bundle_writes_maestro_evidence_1_and_reads_back_equal() {
    let first = "The agent listens on port 7005 by default.";
    let second = "The default port is 7006 — unless the installer finds it taken.";
    let (first_span, second_span) = (span_of(first), span_of(second));
    let expected = json!({
        "schema": "maestro-evidence/1",
        "collection": "ctm",
        "generation": 7,
        "query": "Which port does the agent listen on?",
        "lang": "en",
        "routes": {
            "bm25": "ok",
            "dense": "ok",
            "identifier": {"unavailable": "timed out after 400 ms"},
            "rerank": "ok",
        },
        "passages": [
            {
                "n": 1,
                "section_id": "sec-prerequisites",
                "doc_id": "doc-a",
                "revision_id": "rev-a",
                "title": "Installing the agent",
                "section_path": ["Installing the agent", "Prerequisites"],
                "version": "2.1.0",
                "source_ref": SOURCE_REF,
                "span": [first_span.start, first_span.end],
                "digest": digest_of(first),
                "text": first,
                "alternates": [{"version": "2.0.0", "section_id": "sec-prerequisites-2-0"}],
            },
            {
                "n": 2,
                "doc_id": "doc-a",
                "revision_id": "rev-a",
                "title": "Installing the agent",
                "section_path": [],
                "source_ref": SOURCE_REF,
                "span": [second_span.start, second_span.end],
                "digest": digest_of(second),
                "text": second,
                "alternates": [],
            },
        ],
        "conflicts": [{"entity": "agent", "attribute": "default port", "passages": [1, 2]}],
        "known_gaps": ["no passage states the port of version 2.0.0"],
        "budget": {"evidence_tokens": 41, "limit": 6000},
        "trace": [
            {"n": 1, "score": 0.83, "routes": ["bm25", "dense"], "procedural": false},
            {"n": 2, "score": 0.41, "routes": ["dense"], "procedural": true},
        ],
    });
    let text = write(&bundle()).unwrap();
    assert!(
        text.starts_with(r#"{"schema":"maestro-evidence/1","#),
        "{text}"
    );
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), expected);
    assert_eq!(serde_json::from_str::<Bundle>(&text).unwrap(), bundle());
}

#[test]
fn a_bundle_names_an_unavailable_route_with_its_reason_and_its_known_gaps() {
    let value = serde_json::to_value(bundle()).unwrap();
    assert_eq!(
        value["routes"]["identifier"],
        json!({"unavailable": "timed out after 400 ms"})
    );
    assert_eq!(
        value["known_gaps"],
        json!(["no passage states the port of version 2.0.0"])
    );
    assert_eq!(read(&value).unwrap(), bundle());
    let mut unexplained = value.clone();
    unexplained["routes"]["identifier"] = json!("unavailable");
    assert!(
        read(&unexplained).is_err(),
        "a route that could not run says why"
    );
    let mut unknown = value;
    unknown["routes"]["identifier"] = json!("down");
    let refusal = read(&unknown).unwrap_err();
    assert!(refusal.contains("unknown variant `down`"), "{refusal}");
}

#[test]
fn routes_read_only_from_an_object_that_names_each_route_once() {
    let text = serde_json::to_string(&bundle()).unwrap();
    let twice = text.replacen(
        r#""routes":{"#,
        r#""routes":{"rerank":{"unavailable":"the reranker had no room to load"},"#,
        1,
    );
    assert_ne!(twice, text);
    let refusal = serde_json::from_str::<Bundle>(&twice)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("the route rerank is given twice"),
        "{refusal}"
    );
    let mut listed = serde_json::to_value(bundle()).unwrap();
    listed["routes"] = json!([["rerank", "ok"]]);
    let refusal = read(&listed).unwrap_err();
    assert!(
        refusal.contains("expected an object of routes"),
        "{refusal}"
    );
}

#[test]
fn a_route_that_could_not_run_without_a_reason_is_refused() {
    let original = serde_json::to_value(bundle()).unwrap();
    for reason in ["", " \t"] {
        let mut value = original.clone();
        value["routes"]["identifier"] = json!({"unavailable": reason});
        let refusal = read(&value).unwrap_err();
        assert!(
            refusal.contains("the route identifier could not run and gives no reason"),
            "{refusal}"
        );
    }
}

#[test]
fn a_trace_has_no_score_when_the_reranker_could_not_run() {
    let mut unranked = bundle();
    unranked.routes.insert(
        "rerank".to_owned(),
        RouteStatus::Unavailable("the reranker had no room to load".to_owned()),
    );
    for trace in &mut unranked.trace {
        trace.score = None;
    }
    let value = serde_json::to_value(&unranked).unwrap();
    assert_eq!(
        value["trace"][0],
        json!({"n": 1, "routes": ["bm25", "dense"], "procedural": false})
    );
    assert_eq!(read(&value).unwrap(), unranked);
}

#[test]
fn a_score_reads_back_to_the_same_number() {
    let mut scored = bundle();
    for (trace, score) in scored.trace.iter_mut().zip(SCORES) {
        trace.score = Some(score);
    }
    let text = serde_json::to_string(&scored).unwrap();
    assert_eq!(serde_json::from_str::<Bundle>(&text).unwrap(), scored);
}

/// Scores that `serde_json` reads back one unit in the last place off unless
/// it parses floats with full precision, its `float_roundtrip` feature.
const SCORES: [f64; 2] = [0.385_957_716_695_298_44, 0.923_882_912_051_078_5];

#[test]
fn a_bundle_of_another_schema_or_of_none_is_refused() {
    let mut value = serde_json::to_value(bundle()).unwrap();
    value["schema"] = json!("maestro-evidence/2");
    let refusal = read(&value).unwrap_err();
    assert!(refusal.contains("maestro-evidence/2"), "{refusal}");
    value.as_object_mut().unwrap().remove("schema");
    let refusal = read(&value).unwrap_err();
    assert!(refusal.contains("missing field `schema`"), "{refusal}");
}

#[test]
fn an_unknown_key_is_refused_wherever_it_appears() {
    let original = serde_json::to_value(bundle()).unwrap();
    let objects = [
        "",
        "/passages/0",
        "/passages/0/alternates/0",
        "/conflicts/0",
        "/budget",
        "/trace/0",
    ];
    for pointer in objects {
        let mut value = original.clone();
        let object = value.pointer_mut(pointer).unwrap().as_object_mut().unwrap();
        object.insert("extra".to_owned(), json!(1));
        let refusal = read(&value).unwrap_err();
        assert!(
            refusal.contains("unknown field `extra`"),
            "{pointer}: {refusal}"
        );
    }
}

#[test]
fn a_span_that_starts_after_it_ends_is_refused() {
    let mut value = serde_json::to_value(bundle()).unwrap();
    value["passages"][0]["span"] = json!([30, 20]);
    let refusal = read(&value).unwrap_err();
    assert!(refusal.contains("[30, 20)"), "{refusal}");
    value["passages"][0]["span"] = json!([20, 20]);
    assert_eq!(
        read(&value).unwrap().passages[0].span,
        Span { start: 20, end: 20 }
    );
}

#[test]
fn a_digest_reads_only_as_sha256_and_64_lowercase_hexadecimal_characters() {
    let original = serde_json::to_value(bundle()).unwrap();
    let hex = Digest::of(b"x").as_str().to_owned();
    let refused = [
        hex.clone(),
        format!("sha512:{hex}"),
        format!("sha256:{}", hex.to_uppercase()),
        format!("sha256:{}", &hex[..63]),
    ];
    for digest in refused {
        let mut value = original.clone();
        value["passages"][0]["digest"] = json!(digest);
        let refusal = read(&value).unwrap_err();
        assert!(refusal.contains(&digest), "{refusal}");
    }
}

#[test]
fn passage_numbers_start_from_one_and_are_unique_but_may_skip() {
    let original = serde_json::to_value(bundle()).unwrap();
    let mut value = original.clone();
    value["passages"][1]["n"] = json!(0);
    let refusal = read(&value).unwrap_err();
    assert!(refusal.contains("passage number 0"), "{refusal}");
    value["passages"][1]["n"] = json!(1);
    let refusal = read(&value).unwrap_err();
    assert!(
        refusal.contains("passage number 1 is given twice"),
        "{refusal}"
    );
    let mut skipping = original;
    skipping["passages"][1]["n"] = json!(3);
    skipping["conflicts"][0]["passages"] = json!([1, 3]);
    skipping["trace"][1]["n"] = json!(3);
    let numbers: Vec<u32> = read(&skipping)
        .unwrap()
        .passages
        .iter()
        .map(|passage| passage.n)
        .collect();
    assert_eq!(numbers, [1, 3]);
}

#[test]
fn a_passage_number_given_twice_is_refused_even_with_another_between() {
    let mut value = serde_json::to_value(bundle()).unwrap();
    let third = value["passages"][1].clone();
    value["passages"].as_array_mut().unwrap().push(third);
    value["passages"][2]["n"] = json!(1);
    let refusal = read(&value).unwrap_err();
    assert!(
        refusal.contains("passage number 1 is given twice"),
        "{refusal}"
    );
}

#[test]
fn a_conflict_naming_fewer_than_two_passages_is_refused() {
    let original = serde_json::to_value(bundle()).unwrap();
    for passages in [json!([]), json!([1]), json!([2, 2])] {
        let mut value = original.clone();
        value["conflicts"][0]["passages"] = passages;
        let refusal = read(&value).unwrap_err();
        assert!(
            refusal.contains("the conflict on agent's default port names fewer than two passages"),
            "{refusal}"
        );
    }
}

#[test]
fn a_trace_naming_one_passage_twice_is_refused() {
    let mut value = serde_json::to_value(bundle()).unwrap();
    value["trace"][1]["n"] = json!(1);
    let refusal = read(&value).unwrap_err();
    assert!(
        refusal.contains("the trace names passage 1 twice"),
        "{refusal}"
    );
}

#[test]
fn a_conflict_or_a_trace_naming_a_passage_the_bundle_does_not_hold_is_refused() {
    let original = serde_json::to_value(bundle()).unwrap();
    let mut conflicting = original.clone();
    conflicting["conflicts"][0]["passages"] = json!([1, 3]);
    let refusal = read(&conflicting).unwrap_err();
    assert!(
        refusal.contains("the conflict on agent's default port names passage 3"),
        "{refusal}"
    );
    let mut traced = original;
    traced["trace"][1]["n"] = json!(3);
    let refusal = read(&traced).unwrap_err();
    assert!(refusal.contains("the trace names passage 3"), "{refusal}");
}

#[test]
fn a_bundle_whose_trace_names_a_passage_it_does_not_hold_is_not_written() {
    let mut dangling = bundle();
    dangling.trace[1].n = 3;
    let refusal = write(&dangling).unwrap_err();
    assert!(
        refusal.contains("the trace names passage 3, which the bundle does not hold"),
        "{refusal}"
    );
}

#[test]
fn a_bundle_whose_conflict_names_a_passage_it_does_not_hold_is_not_written() {
    let mut dangling = bundle();
    dangling.conflicts[0].passages = vec![1, 3];
    let refusal = write(&dangling).unwrap_err();
    assert!(
        refusal.contains("the conflict on agent's default port names passage 3"),
        "{refusal}"
    );
}

#[test]
fn a_bundle_with_a_score_that_is_not_a_finite_number_is_not_written() {
    for score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut unscorable = bundle();
        unscorable.trace[1].score = Some(score);
        let refusal = write(&unscorable).unwrap_err();
        assert!(
            refusal.contains(&format!("the trace scores passage 2 {score}, which is not")),
            "{refusal}"
        );
    }
}

#[test]
fn a_bundle_with_a_span_that_starts_after_it_ends_is_not_written() {
    let mut reversed = bundle();
    reversed.passages[0].span = Span { start: 30, end: 20 };
    let refusal = write(&reversed).unwrap_err();
    assert!(
        refusal.contains("the span [30, 20) starts after it ends"),
        "{refusal}"
    );
}
