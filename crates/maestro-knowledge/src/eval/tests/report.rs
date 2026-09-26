//! A report is a strict JSON artifact, `maestro-eval-report/1`: it writes and
//! reads back equal, its numbers to the bit, and its reader refuses what its
//! contract does not name.

use super::support::{
    COLLECTION, GENERATION, bundle, bundle_with, documents, hit, question, sections, whole,
};
use crate::eval::{Report, Schema, judge::judge, metric::measure};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A report of three questions: one found second, one not retrieved while a
/// route could not run, a degraded search, and one unanswerable and left
/// empty.
fn report() -> Report {
    let degraded = [
        ("bm25", None),
        ("dense", Some("no room for the embedder")),
        ("rerank", None),
    ];
    let questions = vec![
        judge(
            &question("second", true),
            &sections(&["wanted"]),
            &bundle(&[hit(1, "other", 0.7), hit(2, "wanted", 0.1)]),
            1234,
        ),
        judge(
            &question("missing", true),
            &sections(&["lost", "gone"]),
            &bundle_with(&[hit(1, "other", 0.3)], &degraded),
            987,
        ),
        judge(&question("refused", false), &[], &bundle(&[]), 55),
    ];
    Report {
        schema: Schema::V1,
        suite: "synthetic".to_owned(),
        collection: COLLECTION.to_owned(),
        generation: GENERATION,
        profiles: BTreeMap::from([
            ("chunk".to_owned(), "structural-500-700/1".to_owned()),
            ("sparse".to_owned(), "bm25-en-fr/1".to_owned()),
        ]),
        seed: u64::MAX,
        degraded_searches: 1,
        metrics: measure(&questions, u64::MAX),
        questions,
    }
}

/// The report's JSON, as a value a test can change.
fn written() -> Value {
    serde_json::to_value(report()).unwrap()
}

/// The refusal of `value` as a report, in words.
fn refusal(value: &Value) -> String {
    value.to_string().parse::<Report>().unwrap_err().to_string()
}

#[test]
fn a_report_writes_and_reads_back_equal() {
    let report = report();
    let text = serde_json::to_string(&report).unwrap();
    assert_eq!(text.parse::<Report>().unwrap(), report);
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema"], "maestro-eval-report/1");
    assert_eq!(value["seed"], json!(u64::MAX));
    assert_eq!(value["degraded_searches"], 1);
    assert_eq!(value["questions"][1]["degraded"], true);
    assert_eq!(
        value["questions"][0]["expected"],
        json!([{"document_id": "document", "section_id": "wanted", "rank": 2}])
    );
    assert_eq!(
        value["questions"][1]["failures"][0],
        json!({
            "cutoff": 5, "class": "not_retrieved", "missed_by": ["bm25"],
            "unavailable": {"dense": "no room for the embedder"},
        })
    );
    assert_eq!(
        value["questions"][2],
        json!({
            "id": "refused", "answerable": false, "expected": [], "passages": 0,
            "latency_us": 55, "degraded": false, "failures": [],
        })
    );
    assert!(value["metrics"]["recall_at_5"]["value"].is_number());
}

#[test]
fn a_document_expected_whole_is_written_without_a_section() {
    let mut report = report();
    report.questions[0] = judge(
        &question("whole", true),
        &documents(&["article", "unfound"]),
        &bundle(&[whole(1, "article", 0.4)]),
        12,
    );
    report.metrics = measure(&report.questions, u64::MAX);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(
        value["questions"][0]["expected"],
        json!([{"document_id": "article", "rank": 1}, {"document_id": "unfound"}])
    );
    assert_eq!(value.to_string().parse::<Report>().unwrap(), report);
    let mut value = written();
    let expected = &mut value["questions"][0]["expected"][0];
    expected.as_object_mut().unwrap().remove("document_id");
    assert!(refusal(&value).contains("missing field `document_id`"));
}

#[test]
fn a_metric_that_covers_no_question_is_left_out() {
    let mut report = report();
    report.questions.retain(|question| question.answerable);
    report.metrics = measure(&report.questions, 1);
    let value = serde_json::to_value(&report).unwrap();
    assert!(value["metrics"].get("no_answer_accuracy").is_none());
    assert_eq!(value.to_string().parse::<Report>().unwrap(), report);
}

#[test]
fn an_unknown_key_is_refused_at_every_level() {
    let paths: [&[&str]; 6] = [
        &[],
        &["questions", "0"],
        &["questions", "0", "expected", "0"],
        &["questions", "1", "failures", "0"],
        &["metrics"],
        &["metrics", "recall_at_10"],
    ];
    for path in paths {
        let mut value = written();
        let target = path.iter().fold(&mut value, |value, key| {
            if let Ok(index) = key.parse::<usize>() {
                &mut value[index]
            } else {
                &mut value[*key]
            }
        });
        target["surplus"] = json!(1);
        assert!(
            refusal(&value).contains("unknown field `surplus`"),
            "{path:?}"
        );
    }
}

#[test]
fn a_key_given_twice_is_refused() {
    let text = serde_json::to_string(&report()).unwrap();
    for (key, twice) in [
        (
            r#""profiles":{"#,
            r#""profiles":{"chunk":"structural-500-700/1","#,
        ),
        (r#""unavailable":{"#, r#""unavailable":{"dense":"again","#),
        (r#""seed":"#, r#""seed":1,"seed":"#),
    ] {
        let repeated = text.replacen(key, twice, 1);
        assert_ne!(repeated, text, "{key}");
        let error = repeated.parse::<Report>().unwrap_err().to_string();
        assert!(
            error.contains("given twice") || error.contains("duplicate field"),
            "{key}: {error}"
        );
    }
}

#[test]
fn another_schema_is_refused() {
    let mut value = written();
    value["schema"] = json!("maestro-eval-report/2");
    assert!(refusal(&value).contains("maestro-eval-report/2"));
    value["schema"] = json!({"maestro-eval-report/1": null});
    assert!(refusal(&value).contains("invalid type: map"));
}

#[test]
fn an_object_written_as_an_array_is_refused() {
    let mut value = written();
    let question = value["questions"][2].clone();
    value["questions"][2] = json!([
        question["id"],
        question["answerable"],
        question["expected"],
        question["passages"],
        question["latency_us"],
        question["degraded"],
        question["failures"],
    ]);
    assert!(refusal(&value).contains("expected a JSON object"));
    let mut value = written();
    value["metrics"]["mrr_at_10"] = json!([0.5, 0.25, 0.75]);
    assert!(refusal(&value).contains("expected a JSON object"));
}

#[test]
fn a_named_value_written_otherwise_is_refused() {
    let mut value = written();
    value["questions"][1]["failures"][0]["class"] = json!({"not_retrieved": null});
    assert!(refusal(&value).contains("invalid type: map"));
    value["questions"][1]["failures"][0]["class"] = json!("wrong_answer");
    assert!(refusal(&value).contains("wrong_answer"));
}

#[test]
fn routes_and_profiles_are_read_from_objects_of_texts_only() {
    let mut value = written();
    value["profiles"] = json!([["chunk", "structural-500-700/1"]]);
    assert!(refusal(&value).contains("an object of names and texts"));
    let mut value = written();
    value["questions"][1]["failures"][0]["unavailable"] = json!({"dense": 1});
    assert!(refusal(&value).contains("invalid type: integer"));
}

#[test]
fn a_metric_written_null_is_refused() {
    let mut value = written();
    value["metrics"]["ndcg_at_10"] = Value::Null;
    assert!(refusal(&value).contains("expected a JSON object"));
}

#[test]
fn a_report_is_one_json_object_and_nothing_after_it() {
    let text = serde_json::to_string(&report()).unwrap();
    assert!(format!("{text} {{}}").parse::<Report>().is_err());
    assert!(format!("[{text}]").parse::<Report>().is_err());
}
