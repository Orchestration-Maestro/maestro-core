//! The label checker: a frozen set passes with only aggregates, IDs and
//! digests, and each broken label is refused with a fixed code, its line and
//! its safe ID, never its text.

use super::support::{
    REVISION, SOURCE, accepted, anchor, answerable, chain, check, labels, link, located, retries,
    source, suite, text, unanswerable,
};
use crate::eval::graph::{CheckError, LabelCode, LabelError, QuestionKind, Stage, check_labels};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::convert::Infallible;

/// The refusal `labels` get at `stage`.
fn refusal(labels: &[Value], stage: Stage) -> LabelError {
    match check(labels, stage) {
        Err(CheckError::Label(error)) => error,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Asserts `labels` are refused, when frozen, with `code` at `line` for `item`.
fn refused(labels: &[Value], code: LabelCode, line: usize, item: Option<&str>) {
    let error = refusal(labels, Stage::Frozen);
    assert_eq!(
        (error.code, error.line, error.item.as_deref()),
        (code, line, item)
    );
}

#[test]
fn a_frozen_set_passes_with_its_aggregates_and_resolved_proofs() {
    let checked = check(&labels(), Stage::Frozen).unwrap();
    let summary = &checked.summary;
    assert_eq!(
        (summary.items, summary.answerable, summary.unanswerable),
        (3, 2, 1)
    );
    assert_eq!(
        (summary.families, summary.links, summary.anchors),
        (3, 3, 3)
    );
    assert_eq!(summary.unreviewed, 0);
    assert_eq!(summary.digest, Digest::of(text(&labels()).as_bytes()));
    assert_eq!(summary.kinds.get("multi_hop"), Some(&1));
    assert_eq!(summary.kinds.get("unanswerable"), Some(&1));
    assert_eq!(
        (summary.languages.get("en"), summary.languages.get("fr")),
        (Some(&2), Some(&1))
    );
    let chain = &checked.items[1];
    assert_eq!(
        (chain.id.as_str(), chain.kind),
        ("q-2", QuestionKind::MultiHop)
    );
    assert_eq!(
        chain.proofs,
        vec![vec![
            located("The relay requires the lantern."),
            located("The lantern is part of the beacon."),
        ]]
    );
    assert!(checked.items[2].proofs.is_empty());
}

#[test]
fn a_digest_mismatch_is_refused_before_any_line_is_read() {
    let text = text(&labels());
    let other = Digest::of(b"another file");
    let Err(CheckError::Label(error)) = check_labels(
        &suite(),
        &text,
        &other,
        Stage::Frozen,
        |reference, digest| Ok::<_, Infallible>(source(reference, digest)),
    ) else {
        panic!("expected a digest refusal");
    };
    assert_eq!(
        (error.code, error.line, error.item),
        (LabelCode::DigestMismatch, 0, None)
    );
}

#[test]
fn a_duplicate_family_is_refused_at_its_second_item() {
    let mut labels = labels();
    labels[1]["family"] = json!("f-1");
    refused(&labels, LabelCode::DuplicateFamily, 2, Some("q-2"));
}

#[test]
fn a_false_exact_quote_is_refused() {
    let mut labels = labels();
    labels[0]["proofs"][0]["links"][0]["anchors"][0]["quote"] =
        json!(Digest::of(b"| retries | 4 |\n").as_str());
    refused(&labels, LabelCode::QuoteMismatch, 1, Some("q-1"));
}

#[test]
fn a_span_inside_a_character_or_past_the_source_is_refused() {
    let start = SOURCE.find("é").unwrap();
    let mut inside = labels();
    inside[0]["proofs"][0]["links"][0]["anchors"][0]["span"] = json!([start, start + 1]);
    refused(&inside, LabelCode::SpanOffBoundary, 1, Some("q-1"));
    let mut past = labels();
    past[0]["proofs"][0]["links"][0]["anchors"][0]["span"] =
        json!([SOURCE.len(), SOURCE.len() + 1]);
    refused(&past, LabelCode::SpanOutOfRange, 1, Some("q-1"));
    let mut empty = labels();
    empty[0]["proofs"][0]["links"][0]["anchors"][0]["span"] = json!([3, 3]);
    refused(&empty, LabelCode::SpanOutOfRange, 1, Some("q-1"));
}

#[test]
fn an_anchor_in_an_unknown_source_is_refused() {
    let mut labels = labels();
    labels[0]["proofs"][0]["links"][0]["anchors"][0]["original"] =
        json!(Digest::of(b"other").as_str());
    refused(&labels, LabelCode::UnknownSource, 1, Some("q-1"));
}

#[test]
fn a_missing_review_is_refused_when_frozen_and_counted_in_a_draft() {
    let mut labels = labels();
    labels[0].as_object_mut().unwrap().remove("review").unwrap();
    refused(&labels, LabelCode::MissingReview, 1, Some("q-1"));
    let draft = check(&labels, Stage::Draft).unwrap();
    assert_eq!(draft.summary.unreviewed, 1);
}

#[test]
fn a_flag_without_the_owners_ruling_or_rejected_by_the_owner_is_refused() {
    let mut flagged = labels();
    flagged[1]["review"]["disposition"] = json!("flagged");
    refused(&flagged, LabelCode::UnresolvedReview, 2, Some("q-2"));
    flagged[1]["review"]["owner"] = json!("rejected");
    refused(&flagged, LabelCode::RejectedReview, 2, Some("q-2"));
    flagged[1]["review"]["owner"] = json!("accepted");
    assert!(check(&flagged, Stage::Frozen).is_ok());
}

#[test]
fn a_predicate_outside_its_kinds_vocabulary_is_refused() {
    let alias = link(
        "relay",
        "ALIAS_OF",
        "lantern",
        &["The relay requires the lantern."],
    );
    let aliased = [answerable("q-1", "f-1", "relationship", &[vec![alias]])];
    let mut labels = labels();
    labels[0] = aliased[0].clone();
    refused(&labels, LabelCode::Vocabulary, 1, Some("q-1"));
}

#[test]
fn a_multi_hop_proof_must_be_a_connected_chain_of_two_links_or_more() {
    let mut single = labels();
    single[1] = answerable("q-2", "f-2", "multi_hop", &[vec![chain()[0].clone()]]);
    refused(&single, LabelCode::Vocabulary, 2, Some("q-2"));
    let mut broken = labels();
    let mut links = chain();
    links[1]["subject"] = json!("beacon");
    broken[1] = answerable("q-2", "f-2", "multi_hop", &[links]);
    refused(&broken, LabelCode::Vocabulary, 2, Some("q-2"));
}

#[test]
fn a_version_difference_needs_a_version_qualified_link() {
    let mut labels = labels();
    labels[0]["kind"] = json!("version_difference");
    refused(&labels, LabelCode::Vocabulary, 1, Some("q-1"));
    labels[0]["proofs"][0]["links"][0]["version"] = json!("9.0.22");
    assert!(check(&labels, Stage::Frozen).is_ok());
}

#[test]
fn an_unanswerable_label_needs_its_rationale_and_no_proof() {
    let mut silent = labels();
    silent[2]
        .as_object_mut()
        .unwrap()
        .remove("unanswerable_reason");
    refused(&silent, LabelCode::UnanswerableRationale, 3, Some("q-3"));
    let mut blank = labels();
    blank[2]["unanswerable_reason"] = json!("  ");
    refused(&blank, LabelCode::UnanswerableRationale, 3, Some("q-3"));
    let mut proved = labels();
    proved[2]["proofs"] = json!([{"links": [retries()]}]);
    refused(&proved, LabelCode::Answerability, 3, Some("q-3"));
}

#[test]
fn a_label_must_agree_with_its_suite_question() {
    let mut language = labels();
    language[0]["language"] = json!("fr");
    refused(&language, LabelCode::LanguageMismatch, 1, Some("q-1"));
    let mut unknown = labels();
    unknown[0]["id"] = json!("q-9");
    refused(&unknown, LabelCode::UnknownItem, 1, Some("q-9"));
    let mut twice = labels();
    twice.push(unanswerable("q-3", "f-4"));
    refused(&twice, LabelCode::DuplicateItem, 4, Some("q-3"));
    let missing = &labels()[..2];
    refused(missing, LabelCode::MissingLabel, 0, Some("q-3"));
    let mut empty = labels();
    empty[0]["proofs"][0]["links"][0]["anchors"] = json!([]);
    refused(&empty, LabelCode::MissingAnchor, 1, Some("q-1"));
}

#[test]
fn malformed_lines_are_refused_by_line_without_their_text() {
    let secret = "SECRET-QUESTION-TEXT";
    let mut labels = labels();
    labels[0]["question"] = json!(secret);
    let error = refusal(&labels, Stage::Frozen);
    assert_eq!(
        (error.code, error.line, error.item),
        (LabelCode::Malformed, 1, None)
    );
    let mut unsafe_id = super::support::labels();
    unsafe_id[0]["id"] = json!(secret.to_lowercase() + " with spaces");
    let error = refusal(&unsafe_id, Stage::Frozen);
    assert_eq!(
        (error.code, error.line, error.item.clone()),
        (LabelCode::UnsafeId, 1, None)
    );
    assert!(!error.to_string().contains("secret"), "{error}");
    let duplicate_key = text(&super::support::labels()).replacen(
        r#""family":"f-1""#,
        r#""family":"f-1","family":"f-1""#,
        1,
    );
    let Err(CheckError::Label(error)) = check_labels(
        &suite(),
        &duplicate_key,
        &Digest::of(duplicate_key.as_bytes()),
        Stage::Frozen,
        |reference, digest| Ok::<_, Infallible>(source(reference, digest)),
    ) else {
        panic!("expected a duplicate-key refusal");
    };
    assert_eq!((error.code, error.line), (LabelCode::Malformed, 1));
}

#[test]
fn a_refusal_prints_its_fixed_code_line_and_id_only() {
    let mut labels = labels();
    labels[0]["proofs"][0]["links"][0]["anchors"][0] = anchor("| label | café |\n");
    labels[0]["proofs"][0]["links"][0]["anchors"][0]["quote"] = json!(Digest::of(b"x").as_str());
    let error = refusal(&labels, Stage::Frozen);
    assert_eq!(error.to_string(), "quote_mismatch at line 1, item q-1");
    assert_eq!(LabelCode::QuoteMismatch.as_str(), "quote_mismatch");
    let unanswered = answerable("q-1", "f-1", "relationship", &[]);
    let mut labels = super::support::labels();
    labels[0] = unanswered;
    let error = refusal(&labels, Stage::Frozen);
    assert_eq!(error.to_string(), "answerability at line 1, item q-1");
    drop((REVISION, accepted()));
}

#[test]
fn a_source_error_is_passed_on_as_is() {
    let text = text(&labels());
    let result = check_labels(
        &suite(),
        &text,
        &Digest::of(text.as_bytes()),
        Stage::Frozen,
        |_, _| Err("the store is locked"),
    );
    assert!(matches!(
        result,
        Err(CheckError::Source("the store is locked"))
    ));
}

#[test]
fn unknown_review_dispositions_and_owner_rulings_are_not_acceptance() {
    for (field, value) in [("disposition", "unknown"), ("owner", "unknown")] {
        let mut labels = labels();
        labels[0]["review"][field] = json!(value);
        refused(&labels, LabelCode::Malformed, 1, None);
    }
}

#[test]
fn unsafe_suite_ids_never_enter_missing_label_errors() {
    let mut suite = suite();
    suite.questions[0].id = "SENSITIVE sentinel\ntext".to_owned();
    let Err(CheckError::Label(error)) =
        check_labels(&suite, "", &Digest::of(b""), Stage::Frozen, |_, _| {
            Ok::<_, Infallible>(None)
        })
    else {
        panic!("expected safe refusal")
    };
    assert!(!error.to_string().contains("SENSITIVE"));
    assert_eq!(error.code, LabelCode::UnsafeId);
    assert_eq!(error.item, None);
}

#[test]
fn dependency_labels_accept_only_dependency_predicates() {
    for predicate in ["DEPENDS_ON", "REQUIRES"] {
        let mut labels = labels();
        let mut dependency = chain()[0].clone();
        dependency["predicate"] = json!(predicate);
        labels[1] = answerable("q-2", "f-2", "dependency", &[vec![dependency]]);
        let checked = check(&labels, Stage::Frozen).unwrap();
        assert_eq!(checked.summary.kinds.get("dependency"), Some(&1));
        assert_eq!(checked.items[1].kind, QuestionKind::Dependency);
    }
    for predicate in ["PART_OF", "DEFAULTS_TO", "INTRODUCED_IN"] {
        let mut labels = labels();
        labels[1]["kind"] = json!("dependency");
        labels[1]["proofs"][0]["links"][0]["predicate"] = json!(predicate);
        refused(&labels, LabelCode::Vocabulary, 2, Some("q-2"));
    }
}

#[test]
fn multi_hop_rejects_default_and_version_hops() {
    for predicate in [
        "DEFAULTS_TO",
        "INTRODUCED_IN",
        "DEPRECATED_IN",
        "REPLACES",
        "APPLIES_TO",
    ] {
        let mut labels = labels();
        labels[1]["proofs"][0]["links"][0]["predicate"] = json!(predicate);
        refused(&labels, LabelCode::Vocabulary, 2, Some("q-2"));
    }
}
