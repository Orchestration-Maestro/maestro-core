//! The expansion guard: quantities in any spelling, protected constraint
//! words in English and French, no interface check, exact size bounds, and
//! a typed reason for every rejection.

use crate::{
    query::understand,
    search::{
        hyde::parse_reply,
        intent::{Expansion, ExpansionFailure, IntentTrigger},
        intent_guard::validate,
    },
};

/// The expansion `reply` gives for `question`, or the rule it breaks.
pub(super) fn guarded(question: &str, reply: &str) -> Result<Expansion, ExpansionFailure> {
    let expansion = parse_reply(reply)?;
    validate(&understand(question), &expansion)?;
    Ok(expansion)
}

fn reply(passage: &str, keywords: &str) -> String {
    serde_json::json!({ "passage": passage, "keywords": keywords }).to_string()
}

fn accepted(question: &str, passage: &str, keywords: &str) -> bool {
    guarded(question, &reply(passage, keywords)).is_ok()
}

#[test]
fn a_numeral_and_the_questions_ordinal_are_one_quantity() {
    let question = "notify me only after the third failure";
    for passage in [
        "Notify only after the 3rd failure of the job.",
        "Notify only after 3 failures of the job.",
        "Notify only after the third failure of the job.",
    ] {
        assert!(
            accepted(question, passage, "number of failures"),
            "{passage}"
        );
    }
    for passage in [
        "Notify only after the 2nd failure.",
        "Notify only after 3x failure.",
        "Notify only after 4 failures.",
        "Notify after the third failure.",
    ] {
        assert!(!accepted(question, passage, "failures"), "{passage}");
    }
    assert!(!accepted(
        question,
        "Notify only after the third failure.",
        "5 retries"
    ));
}

#[test]
fn an_added_interface_or_platform_is_no_longer_rejected() {
    for added in [
        "In the Monitoring domain of the Web interface",
        "With the API or the CLI",
        "On Windows or Linux",
    ] {
        let passage = format!("{added}, rerun a failed job.");
        assert!(
            accepted("rerun a failed job", &passage, "rerun monitoring planning"),
            "{passage}"
        );
    }
}

#[test]
fn an_added_negation_votes_but_a_dropped_one_is_rejected() {
    assert!(accepted(
        "run a job now",
        "Run the job now, without any delay; it must not be held.",
        "run now"
    ));
    assert_eq!(
        guarded(
            "rerun a job without confirmation",
            &reply("Rerun a job with a confirmation.", "rerun")
        ),
        Err(ExpansionFailure::ProtectedMissing)
    );
}

#[test]
fn french_constraint_words_are_protected() {
    let question = "relancer un job seulement après trois échecs";
    for passage in [
        "Relancer un job seulement après trois échecs.",
        "Relancer un job seulement après 3 échecs.",
        "Relancer un job seulement après le 3e échec, jamais avant.",
    ] {
        assert!(accepted(question, passage, "relance échecs"), "{passage}");
    }
    for passage in [
        "Relancer un job après trois échecs.",
        "Relancer un job seulement après deux échecs.",
    ] {
        assert!(!accepted(question, passage, "relance"), "{passage}");
    }
    assert!(!accepted(
        "relancer un job sans confirmation",
        "Relancer un job avec confirmation.",
        "relance"
    ));
}

#[test]
fn size_bounds_accept_exactly_their_limits() {
    let question = "scheduler job";
    let passage = format!("scheduler job {}", "a".repeat(2048 - 14));
    assert_eq!(passage.len(), 2048);
    assert!(accepted(question, &passage, "count"));
    assert!(!accepted(question, &format!("{passage}a"), "count"));
    let keywords = format!("count {}", "k".repeat(512 - 6));
    assert_eq!(keywords.len(), 512);
    assert!(accepted(question, "scheduler job", &keywords));
    assert!(!accepted(
        question,
        "scheduler job",
        &format!("{keywords}k")
    ));
    let base = reply("scheduler job", "count");
    let padded = format!("{base}{}", " ".repeat(4096 - base.len()));
    assert!(guarded(question, &padded).is_ok());
    assert!(guarded(question, &format!("{padded} ")).is_err());
}

#[test]
fn each_rejection_names_its_rule() {
    let question = "retry ERR-042 only after the third failure";
    let keep = "Retry ERR-042 only after the third failure.";
    for (reply, failure) in [
        ("not json".to_owned(), ExpansionFailure::Malformed),
        (
            r#"{"passage":"x","keywords":"y","extra":1}"#.to_owned(),
            ExpansionFailure::Malformed,
        ),
        (reply(keep, " "), ExpansionFailure::Empty),
        (reply(" ", "retry"), ExpansionFailure::Empty),
        (" ".repeat(4097), ExpansionFailure::Oversize),
        (
            reply(&"a".repeat(2049), "retry"),
            ExpansionFailure::Oversize,
        ),
        (reply(keep, &"k".repeat(513)), ExpansionFailure::Oversize),
        (
            reply("Retry ERR-042 after the third failure.", "retry"),
            ExpansionFailure::ProtectedMissing,
        ),
        (reply(keep, "retry 7 times"), ExpansionFailure::AddedNumber),
        (
            reply("Retry err 042 only after the third failure.", "retry"),
            ExpansionFailure::IdentifierMissing,
        ),
        (
            reply(keep, "ERR-042 --force"),
            ExpansionFailure::IdentifierAdded,
        ),
    ] {
        assert_eq!(guarded(question, &reply).map(drop), Err(failure), "{reply}");
    }
    assert!(guarded(question, &reply(keep, "retry")).is_ok());
}

#[test]
fn every_failure_has_its_own_code() {
    let failures = [
        ExpansionFailure::ModelUnavailable,
        ExpansionFailure::DeadlineExceeded,
        ExpansionFailure::Malformed,
        ExpansionFailure::Empty,
        ExpansionFailure::Oversize,
        ExpansionFailure::ProtectedMissing,
        ExpansionFailure::AddedNumber,
        ExpansionFailure::IdentifierMissing,
        ExpansionFailure::IdentifierAdded,
    ];
    let codes = failures.map(ExpansionFailure::code);
    assert_eq!(
        codes,
        [
            "intent_model_unavailable",
            "intent_deadline_exceeded",
            "intent_guard_malformed",
            "intent_guard_empty",
            "intent_guard_oversize",
            "intent_guard_protected_missing",
            "intent_guard_added_number",
            "intent_guard_identifier_missing",
            "intent_guard_identifier_added",
        ]
    );
}

#[test]
fn a_misspelled_trigger_setting_is_refused() {
    let misspelled = serde_json::json!({
        "low_confidence": { "min_top_rerank": 2.0, "min_top_rerank_score": 1.0 }
    });
    assert!(serde_json::from_value::<IntentTrigger>(misspelled).is_err());
}

#[test]
fn a_quoted_constraint_word_is_still_the_word() {
    let question = "notify only after the third failure when the status is not ok";
    for quoted in [
        "'not ok'",
        "\"not ok\"",
        "‘not ok’",
        "“not ok”",
        "«not ok»",
        "« not ok »",
    ] {
        let passage = format!("Notify only after the third failure whose status is {quoted}.");
        assert!(accepted(question, &passage, "failure status"), "{passage}");
    }
    assert_eq!(
        guarded(
            question,
            &reply(
                "Notify only after the third failure whose status is 'ok'.",
                "status"
            )
        ),
        Err(ExpansionFailure::ProtectedMissing)
    );
}

#[test]
fn a_curly_apostrophe_splits_words_like_a_straight_one() {
    let question = "ne pas relancer l'agent d'un job jusqu'à trois échecs";
    for passage in [
        "Ne pas relancer l'agent d'un job jusqu'à trois échecs.",
        "Ne pas relancer l’agent d’un job jusqu’à 3 échecs.",
    ] {
        assert!(accepted(question, passage, "relance agent"), "{passage}");
    }
    assert!(accepted(
        "don’t rerun the job",
        "Don't rerun the job.",
        "rerun"
    ));
}

#[test]
fn french_elision_keeps_the_elided_word_whole() {
    let question = "relancer seulement jusqu'à trois échecs, avant qu'il s'arrête";
    assert!(accepted(
        question,
        "Relancer seulement jusqu'à 3 échecs, avant qu'il s'arrête.",
        "relance"
    ));
    for passage in [
        "Relancer seulement à 3 échecs, avant qu'il s'arrête.",
        "Relancer seulement jusqu'à 3 échecs, avant qu'elle s'arrête.",
    ] {
        assert!(!accepted(question, passage, "relance"), "{passage}");
    }
}
