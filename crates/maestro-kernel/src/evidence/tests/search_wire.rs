//! Backwards-readable search inventory and request-budget fields.

use super::support::bundle;
use crate::evidence::{Bundle, Inventory, RequestBudget};
use serde_json::{Value, json};

/// A bundle with valid search fields, ready to adjust one field at a time.
fn valid_wire() -> Value {
    let mut value = serde_json::to_value(bundle()).unwrap();
    value["routes"]["structured"] = json!("ok");
    value["request_budget"] = json!({"k": 10, "max_tokens": 6000, "deadline_ms": 1500});
    value["inventory"] = json!({
        "kind": "documents_by_set",
        "set_filter": null,
        "total_documents": 2,
        "sets": [
            {"value": "cli", "documents": 1},
            {"value": null, "documents": 1}
        ]
    });
    value
}

#[test]
fn optional_search_fields_round_trip_and_old_bundles_remain_unchanged() {
    assert_eq!(
        RequestBudget::default(),
        RequestBudget {
            k: 10,
            max_tokens: 6000,
            deadline_ms: 1500,
        }
    );
    let old = serde_json::to_value(bundle()).unwrap();
    assert!(old.get("request_budget").is_none());
    assert!(old.get("inventory").is_none());
    assert_eq!(
        serde_json::from_value::<Bundle>(old.clone()).unwrap(),
        bundle()
    );

    let mut value = old;
    value["routes"]["structured"] = json!("ok");
    value["budget"]["evidence_tokens"] = json!(6000);
    value["request_budget"] = json!({"k": 2, "max_tokens": 6000, "deadline_ms": 1500});
    value["inventory"] = json!({
        "kind": "versions",
        "set_filter": null,
        "total_documents": 2,
        "versions": [
            {"value": "2.0", "documents": 1},
            {"value": null, "documents": 1}
        ]
    });
    let decoded: Bundle = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
}

fn assert_invalid(value: Value) {
    assert!(serde_json::from_value::<Bundle>(value).is_err());
}

#[test]
fn request_budget_fields_enforce_their_bounds() {
    let valid = valid_wire();
    for (pointer, replacement) in [
        ("/request_budget/k", json!(0)),
        ("/request_budget/k", json!(51)),
        ("/request_budget/max_tokens", json!(0)),
        ("/request_budget/deadline_ms", json!(10_001)),
        ("/budget/limit", json!(5999)),
        ("/budget/evidence_tokens", json!(6001)),
    ] {
        let mut invalid = valid.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        assert_invalid(invalid);
    }
    let mut too_many_passages = valid.clone();
    too_many_passages["request_budget"]["k"] = json!(1);
    assert_invalid(too_many_passages);
    let mut unknown_key = valid;
    unknown_key["request_budget"]["extra"] = json!(true);
    assert_invalid(unknown_key);
}

#[test]
fn inventories_require_exact_ordered_groups_and_structured_success() {
    let valid = valid_wire();
    for groups in [
        json!([
            {"value": null, "documents": 1},
            {"value": "cli", "documents": 1}
        ]),
        json!([
            {"value": "cli", "documents": 1},
            {"value": "cli", "documents": 1}
        ]),
    ] {
        let mut invalid = valid.clone();
        invalid["inventory"]["sets"] = groups;
        assert_invalid(invalid);
    }
    let mut wrong_total = valid.clone();
    wrong_total["inventory"]["total_documents"] = json!(3);
    assert_invalid(wrong_total);
    let mut unavailable = valid.clone();
    unavailable["routes"]["structured"] = json!({"unavailable": "not routed"});
    assert_invalid(unavailable);
    let mut unknown_inventory_key = valid.clone();
    unknown_inventory_key["inventory"]["extra"] = json!(true);
    assert_invalid(unknown_inventory_key);
    let mut unknown_group_key = valid;
    unknown_group_key["inventory"]["sets"][0]["extra"] = json!(true);
    assert_invalid(unknown_group_key);
}

#[test]
fn complete_inventory_validation_requires_exact_counts() {
    let inventory: Inventory = serde_json::from_value(json!({
        "kind": "documents_by_set",
        "set_filter": null,
        "total_documents": 2,
        "sets": [{"value": "cli", "documents": 1}]
    }))
    .expect("inventory data");
    assert!(inventory.validate().is_err());
}

#[test]
fn partial_inventory_counts_require_an_explicit_truncation_gap() {
    let mut partial = valid_wire();
    partial["inventory"]["sets"] = json!([{"value": "cli", "documents": 1}]);
    assert_invalid(partial.clone());

    partial["known_gaps"] = json!([
        "Search inventory truncated: 1 groups were omitted; remaining entries are incomplete."
    ]);
    assert!(serde_json::from_value::<Bundle>(partial.clone()).is_ok());

    partial["inventory"]["sets"][0]["documents"] = json!(3);
    assert_invalid(partial);

    let mut exact_partial = valid_wire();
    exact_partial["known_gaps"] = json!(["Search inventory truncated: 1 group omitted"]);
    exact_partial["inventory"]["sets"] = json!([
        {"value": "cli", "documents": 2}
    ]);
    assert!(serde_json::from_value::<Bundle>(exact_partial).is_ok());
}

#[test]
fn inventories_accept_the_exact_response_bounds_and_refuse_overflow() {
    let mut max_groups = valid_wire();
    max_groups["inventory"]["sets"] = Value::Array(
        (0..1000)
            .map(|index| json!({"value": format!("{index:05}"), "documents": 1}))
            .collect(),
    );
    max_groups["inventory"]["total_documents"] = json!(1000);
    assert!(
        serde_json::to_vec(&max_groups["inventory"]).unwrap().len() < 32_768,
        "the group-count boundary must not also hit the serialized-size bound"
    );
    assert!(serde_json::from_value::<Bundle>(max_groups).is_ok());

    let mut too_many_groups = valid_wire();
    too_many_groups["inventory"]["sets"] = Value::Array(
        (0..=1000)
            .map(|index| json!({"value": format!("{index:05}"), "documents": 1}))
            .collect(),
    );
    too_many_groups["inventory"]["total_documents"] = json!(1001);
    assert_invalid(too_many_groups);

    let mut exact_size = json!({
        "kind": "documents_by_set",
        "set_filter": null,
        "total_documents": 1,
        "sets": [{"value": "", "documents": 1}]
    });
    let empty_size = serde_json::to_vec(&exact_size).unwrap().len();
    exact_size["sets"][0]["value"] = json!("x".repeat(32_768 - empty_size));
    assert_eq!(serde_json::to_vec(&exact_size).unwrap().len(), 32_768);
    let mut at_size_limit = valid_wire();
    at_size_limit["inventory"] = exact_size;
    assert!(serde_json::from_value::<Bundle>(at_size_limit).is_ok());

    let mut too_large = valid_wire();
    too_large["inventory"]["sets"] = json!([{
        "value": "x".repeat(32_769),
        "documents": 1
    }]);
    assert_invalid(too_large);
}
