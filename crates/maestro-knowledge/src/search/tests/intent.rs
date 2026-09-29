//! Guarded additive intent expansion contracts.

use super::intent_guard::guarded;
use crate::search::{SearchConfiguration, intent::IntentExpansion};

#[test]
fn intent_is_off_by_default_and_configuration_stays_copy() {
    fn copy<T: Copy>(value: T) -> (T, T) {
        (value, value)
    }
    let (left, right) = copy(SearchConfiguration::default());
    assert_eq!(left, right);
    assert_eq!(left.intent_expansion, IntentExpansion::Off);
}

#[test]
fn intent_guard_preserves_constraints() {
    let question = "notify only after the third failure from a step without retry";
    let good = concat!(
        r#"{"passage":"Notify only after the third failure from a step without retry. "#,
        r#"Configure the failure count condition and notification action.","#,
        r#""keywords":"failure count notification"}"#,
    );
    assert!(guarded(question, good).is_ok());
    for changed in [
        good.replace("third", "second"),
        good.replace("from a step", "a step"),
        good.replace("without", "with"),
    ] {
        assert!(guarded(question, &changed).is_err(), "{changed}");
    }
    for invalid in ["", "{}", "not json", r#"{"passage":"","keywords":"count"}"#] {
        assert!(guarded(question, invalid).is_err());
    }
}

#[test]
fn intent_guard_preserves_exact_identifiers_and_bounded_inputs() {
    let question = "retry ERR-042 after 3 failures";
    let reply = |passage: &str, keywords: &str| {
        serde_json::json!({"passage":passage,"keywords":keywords}).to_string()
    };
    assert!(
        guarded(
            question,
            &reply(
                "Retry ERR-042 after 3 failures with a count condition",
                "failure count"
            )
        )
        .is_ok()
    );
    for (passage, keywords) in [
        ("Retry ERR-043 after 3 failures", "count"),
        ("Retry ERR-042 after 2 failures", "count"),
        ("Retry ERR-042 after 3 failures", "ERR-099"),
    ] {
        assert!(guarded(question, &reply(passage, keywords)).is_err());
    }
    assert!(guarded(question, &reply(&"a".repeat(2049), "count")).is_err());
    assert!(
        guarded(
            question,
            &reply("Retry ERR-042 after 3 failures", &"a".repeat(513))
        )
        .is_err()
    );
    assert!(guarded(question, &" ".repeat(4097)).is_err());
}

#[test]
fn intent_votes_are_additive_and_use_their_own_weight() {
    use crate::search::{Hit, Route, RouteList, fuse_weighted};
    let configuration = SearchConfiguration {
        intent_weight: 0.5,
        ..SearchConfiguration::default()
    };
    let lists = [
        (Route::Dense, "original"),
        (Route::Lexical, "original"),
        (Route::DenseIntent, "expanded"),
        (Route::LexicalIntent, "expanded"),
    ]
    .map(|(route, id)| RouteList {
        route,
        hits: vec![Hit {
            chunk_id: id.to_owned(),
            score: 100.0,
        }],
    });
    let fused = fuse_weighted(&lists, 10, configuration.rrf_k, |route| {
        configuration.weight(route)
    });
    assert_eq!(fused.len(), 2);
    assert_eq!(fused[0].chunk_id, "original");
    assert_eq!(fused[1].chunk_id, "expanded");
    assert!((fused[0].score - 2.0 * fused[1].score).abs() < f64::EPSILON);
    assert_eq!(fused[1].ranks.len(), 2);
}

#[test]
fn intent_accepts_bounded_keyword_arrays_without_relaxing_guards() {
    let question = "retry ERR-042 after 3 failures";
    let mut reply = serde_json::json!({
        "passage":"Retry ERR-042 after 3 failures with a count condition",
        "keywords":["failure count", "notification"]
    });
    let parsed = guarded(question, &reply.to_string()).unwrap();
    assert_eq!(parsed.keywords, "failure count notification");
    for invalid in [
        serde_json::json!([]),
        serde_json::json!([3]),
        serde_json::json!(["x".repeat(513)]),
    ] {
        reply["keywords"] = invalid;
        assert!(guarded(question, &reply.to_string()).is_err());
    }
}

#[test]
fn intent_trigger_is_strictly_below_threshold_and_handles_missing_scores() {
    use crate::search::IntentTrigger;
    let trigger = IntentTrigger::LowConfidence {
        min_top_rerank: 2.0,
    };
    assert!(trigger.should_expand(None));
    assert!(trigger.should_expand(Some(1.99)));
    assert!(!trigger.should_expand(Some(2.0)));
    assert!(!trigger.should_expand(Some(4.0)));
    assert!(IntentTrigger::Always.should_expand(Some(4.0)));
    assert_eq!(
        SearchConfiguration::default().intent_trigger,
        IntentTrigger::Always
    );
}
