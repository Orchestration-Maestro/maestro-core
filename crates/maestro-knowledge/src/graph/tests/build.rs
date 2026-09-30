//! Every frozen build field participates in submission identity.

use crate::graph::build::inputs;
use maestro_kernel::{
    artifact::Digest,
    facts::{Budget, BuildPlan, Provenance},
};
use serde_json::json;

#[test]
fn graph_build_identity_contains_the_complete_ordered_plan() {
    let plan = BuildPlan {
        collection_id: "graph".into(),
        provenance: Provenance {
            extractor: "rule/1".into(),
            profile: Digest::of(b"profile"),
        },
        sources: vec!["rev-a".into(), "rev-b".into()],
        budget: Budget {
            max_claims: 10,
            max_rejections: 3,
        },
    };
    let frozen = inputs(&plan, None);
    assert_eq!(
        frozen,
        json!({ "collection": "graph", "extractor": "rule/1",
            "profile": plan.provenance.profile.as_str(), "sources": ["rev-a", "rev-b"],
            "max_claims": 10, "max_rejections": 3 })
    );
    assert_eq!(frozen, inputs(&plan.clone(), None));
    for change in 0..6 {
        let mut other = plan.clone();
        match change {
            0 => other.collection_id = "other".into(),
            1 => other.provenance.extractor = "rule/2".into(),
            2 => other.provenance.profile = Digest::of(b"other"),
            3 => other.sources.reverse(),
            4 => other.budget.max_claims += 1,
            _ => other.budget.max_rejections += 1,
        }
        assert_ne!(frozen, inputs(&other, None));
    }
}

#[test]
fn graph_model_inputs_freeze_card_prompt_window_profile_and_token_budget() {
    let plan = BuildPlan {
        collection_id: "graph".into(),
        provenance: Provenance {
            extractor: "model/card-digest".into(),
            profile: Digest::of(b"model profile"),
        },
        sources: vec!["rev-a".into()],
        budget: Budget {
            max_claims: 4,
            max_rejections: 6,
        },
    };
    let frozen = inputs(
        &plan,
        Some(json!({
            "card_digest": "card",
            "prompt_digest": "prompt",
            "window_policy_digest": "window",
            "profile_digest": "profile",
            "token_budget": 4096
        })),
    );
    assert_eq!(frozen["extractor_inputs"]["token_budget"], 4096);
    assert_eq!(frozen["extractor_inputs"]["prompt_digest"], "prompt");
    let changed = inputs(
        &plan,
        Some(json!({
            "card_digest": "card",
            "prompt_digest": "prompt",
            "window_policy_digest": "window",
            "profile_digest": "profile",
            "token_budget": 4097
        })),
    );
    assert_ne!(frozen, changed);
}
