//! Draft refusal conditions are independent; exact budget endpoints are admitted.
use super::draft::{budget, candidate, card, window};
use crate::eval::graph::draft::{
    DraftBudget, DraftError, DraftRequest, check_candidate, checked_chat,
};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[test]
fn candidate_refuses_each_identity_and_question_defect_independently() {
    for defect in ["count", "id", "review", "blank", "expected"] {
        let mut output = candidate();
        let mut label: Value = serde_json::from_str(output["labels"].as_str().unwrap()).unwrap();
        let mut suite: Value = serde_json::from_str(output["suite"].as_str().unwrap()).unwrap();
        match defect {
            "count" => {
                let mut second = suite.clone();
                second["id"] = json!("q-2");
                output["suite"] = json!(format!("{suite}\n{second}"));
            }
            "id" => label["id"] = json!("q-2"),
            "review" => label["review"] = super::support::accepted(),
            "blank" => suite["question"] = json!("   "),
            _ => suite["expected"][0]["source_ref"] = json!("another.md"),
        }
        if defect != "count" {
            output["suite"] = json!(suite.to_string());
        }
        output["labels"] = json!(label.to_string());
        assert_eq!(
            check_candidate(&output.to_string(), &window(), &BTreeSet::new()).err(),
            Some(DraftError::Candidate),
            "{defect}"
        );
    }
}

#[test]
fn draft_source_pin_and_window_validation_precede_candidate_decoding() {
    for defect in ["id", "digest", "span"] {
        let mut window = window();
        match defect {
            "id" => window.id = "unsafe id".into(),
            "digest" => window.original = Digest::of(b"different"),
            _ => window.span = [1, 1],
        }
        assert_eq!(
            check_candidate("not json", &window, &BTreeSet::new()).unwrap_err(),
            DraftError::Source,
            "{defect}"
        );
    }
}

#[test]
fn draft_chat_contains_exact_window_text_not_the_entire_source() {
    let card = card();
    let mut window = window();
    window.span = super::support::span_of("| retries | 3 |\n");
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let chat = checked_chat(&request).unwrap();
    let payload: Value = serde_json::from_str(&chat.messages[1].content).unwrap();
    assert_eq!(payload["source"], "| retries | 3 |\n");
}

#[test]
fn draft_chat_checks_each_zero_limit_and_context_exact_fit() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let mut request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    for limits in [
        DraftBudget {
            input_bytes: 0,
            ..budget()
        },
        DraftBudget {
            output_bytes: 0,
            ..budget()
        },
        DraftBudget {
            deadline_ms: 0,
            ..budget()
        },
        DraftBudget {
            input_tokens: 0,
            ..budget()
        },
        DraftBudget {
            output_tokens: 0,
            ..budget()
        },
        DraftBudget {
            input_tokens: 3073,
            ..budget()
        },
    ] {
        request.budget = limits;
        assert_eq!(
            checked_chat(&request).err(),
            Some(DraftError::Budget),
            "{limits:?}"
        );
    }
    request.budget = DraftBudget {
        input_tokens: 3072,
        ..budget()
    };
    assert!(checked_chat(&request).is_ok());
    let empty = Digest::of(b" ");
    request.prompt = " ";
    request.prompt_digest = &empty;
    assert_eq!(checked_chat(&request).unwrap_err(), DraftError::Prompt);
}

#[test]
fn draft_chat_checks_role_and_model_output_cap_independently() {
    use super::draft::card_with;
    use maestro_kernel::gateway::Role;
    let window = window();
    let prompt = Digest::of(b"draft");
    for (role, cap, expected) in [
        (Role::Answerer, Some(1024), None),
        (Role::Answerer, Some(512), Some(DraftError::Budget)),
        (Role::Answerer, None, Some(DraftError::Budget)),
        (Role::Reranker, Some(1024), Some(DraftError::Card)),
    ] {
        let card = card_with(role, 4096, cap);
        let request = DraftRequest {
            card: &card,
            card_digest: card.digest(),
            prompt: "draft",
            prompt_digest: &prompt,
            window: &window,
            budget: budget(),
        };
        let result = checked_chat(&request);
        if let Some(error) = expected {
            assert_eq!(result.err(), Some(error));
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn draft_rejects_an_anchor_from_another_reference_despite_the_same_digest() {
    let mut output = candidate();
    let mut label: Value = serde_json::from_str(output["labels"].as_str().unwrap()).unwrap();
    label["proofs"][0]["links"][0]["anchors"][0]["source_ref"] = json!("another.md");
    output["labels"] = json!(label.to_string());
    assert_eq!(
        check_candidate(&output.to_string(), &window(), &BTreeSet::new()).err(),
        Some(DraftError::Candidate)
    );
}

#[tokio::test]
async fn draft_accepts_exact_rendered_input_token_budget() {
    use super::draft::DraftModel;
    use crate::eval::graph::draft::draft_window;
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: DraftBudget {
            input_tokens: 2,
            ..budget()
        },
    };
    assert!(
        draft_window(&DraftModel::valid(), &request, &BTreeSet::new())
            .await
            .is_ok()
    );
}

#[test]
fn candidate_identity_refuses_before_outside_anchor_errors() {
    candidate_precedence(false);
}

#[test]
fn candidate_population_refuses_before_outside_anchor_errors() {
    candidate_precedence(true);
}

/// Refusal priority must not depend on an invalid candidate's later anchor defects.
fn candidate_precedence(extra_question: bool) {
    let mut output = candidate();
    if extra_question {
        let mut second: Value = serde_json::from_str(output["suite"].as_str().unwrap()).unwrap();
        second["id"] = json!("q-2");
        output["suite"] = json!(format!("{}\n{second}", output["suite"].as_str().unwrap()));
    } else {
        let mut label: Value = serde_json::from_str(output["labels"].as_str().unwrap()).unwrap();
        label["id"] = json!("q-2");
        output["labels"] = json!(label.to_string());
    }
    let mut window = window();
    window.span = [0, 10];
    assert_eq!(
        check_candidate(&output.to_string(), &window, &BTreeSet::new()).err(),
        Some(DraftError::Candidate),
        "extra_question={extra_question}"
    );
}
