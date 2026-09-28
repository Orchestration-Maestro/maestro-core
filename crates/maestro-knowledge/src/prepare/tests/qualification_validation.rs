use super::super::{QualificationMode, TokenizerQualification};
use super::{qualification::qualification_value, support::MODEL_FILE};
use serde_json::{Value, json};

#[test]
fn qualification_requires_both_special_token_options() {
    for field in ["add_special", "parse_special"] {
        let mut artifact = qualification_value(0, MODEL_FILE);
        artifact[field] = Value::Bool(false);
        assert!(
            TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn qualification_allows_a_single_id_vocabulary_and_rejects_reversed_bounds() {
    let mut equal_bounds = qualification_value(0, MODEL_FILE);
    equal_bounds["vocabulary"] = json!({"minimum_id": 0, "maximum_id": 0});
    for fixture in equal_bounds["fixtures"].as_array_mut().unwrap() {
        let count = fixture["ids"].as_array().unwrap().len();
        let ids = vec![0; count];
        fixture["ids"] = json!(ids);
        fixture["confirmatory_ids"] = json!(vec![0; count]);
    }
    assert!(TokenizerQualification::parse(&serde_json::to_vec(&equal_bounds).unwrap()).is_ok());

    let mut reversed = qualification_value(0, MODEL_FILE);
    let maximum = reversed["vocabulary"]["maximum_id"].as_u64().unwrap();
    reversed["vocabulary"]["minimum_id"] = json!(maximum + 1);
    assert!(TokenizerQualification::parse(&serde_json::to_vec(&reversed).unwrap()).is_err());
}

#[test]
fn qualification_rejects_each_malformed_date_shape() {
    for date in ["2026-09-2", "2026x90-78", "2026-09x27", "202x-09-27"] {
        let mut artifact = qualification_value(0, MODEL_FILE);
        artifact["created"] = json!(date);
        assert!(
            TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).is_err(),
            "{date}"
        );
    }
}

#[test]
fn native_qualification_is_not_synthetic() {
    let mut artifact = qualification_value(0, MODEL_FILE);
    artifact["mode"] = json!("native");
    let profile = TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).unwrap();

    assert_eq!(profile.qualification_mode(), QualificationMode::Native);
    assert!(!profile.is_synthetic());
}

#[test]
fn qualification_rejects_blank_tool_names() {
    let mut artifact = qualification_value(0, MODEL_FILE);
    artifact["native_tool"]["name"] = json!("  ");

    assert!(TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).is_err());
}

#[test]
fn qualification_error_display_preserves_the_rejection_reason() {
    let mut artifact = qualification_value(0, MODEL_FILE);
    artifact["schema"] = json!("unsupported/2");
    let error = TokenizerQualification::parse(&serde_json::to_vec(&artifact).unwrap()).unwrap_err();

    assert_eq!(error.to_string(), "unknown tokenizer qualification schema");
}
