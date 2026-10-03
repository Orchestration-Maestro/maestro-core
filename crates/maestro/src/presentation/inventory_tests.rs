//! Frozen inventory keys and their independently retained English neighbours.
use super::messages::{BUILT_INS, Interface, MessageKey, interpolate};
use serde_json::Value;
use std::collections::BTreeMap;

/// Typed snapshot cases for slice one; the current-source scanner discovers its own keys.
pub(super) fn migrated_keys() -> Vec<(MessageKey, &'static str)> {
    vec![
        (
            MessageKey::CatalogMissingOwnership,
            "catalog_missing_ownership",
        ),
        (MessageKey::CatalogCheckPassed, "catalog_check_passed"),
        (
            MessageKey::CatalogCodeownersRegular,
            "catalog_codeowners_regular",
        ),
        (MessageKey::DiagnosticPath, "diagnostic_path"),
        (
            MessageKey::CatalogCodeownersMissing,
            "catalog_codeowners_missing",
        ),
        (
            MessageKey::CatalogCodeownersPassed,
            "catalog_codeowners_passed",
        ),
        (MessageKey::CatalogCompiled, "catalog_compiled"),
        (MessageKey::CatalogOutputFilename, "catalog_output_filename"),
        (MessageKey::CatalogStagingChanged, "catalog_staging_changed"),
        (MessageKey::CatalogOutputRegular, "catalog_output_regular"),
        (MessageKey::CatalogIndexPassed, "catalog_index_passed"),
        (
            MessageKey::CatalogEvidenceFilename,
            "catalog_evidence_filename",
        ),
        (MessageKey::CatalogEvidenceLimit, "catalog_evidence_limit"),
        (MessageKey::CatalogOwnersPassed, "catalog_owners_passed"),
        (
            MessageKey::CatalogTargetUntrusted,
            "catalog_target_untrusted",
        ),
        (MessageKey::CatalogTargetApprove, "catalog_target_approve"),
        (MessageKey::DiagnosticInstruction, "diagnostic_instruction"),
        (MessageKey::CatalogTargetDeclined, "catalog_target_declined"),
        (MessageKey::OutputWriteFailed, "output_write_failed"),
        (MessageKey::RunRepairDispatch, "run_repair_dispatch"),
        (MessageKey::RunRetrievalDispatch, "run_retrieval_dispatch"),
        (MessageKey::RunPrepareModelsOff, "run_prepare_models_off"),
        (MessageKey::RunPublishModelsOff, "run_publish_models_off"),
        (MessageKey::RunModelDispatch, "run_model_dispatch"),
        (
            MessageKey::SessionWorkspaceDirectory,
            "session_workspace_directory",
        ),
        (
            MessageKey::SessionBackendUnavailable,
            "session_backend_unavailable",
        ),
        (MessageKey::TrustHomeUnavailable, "trust_home_unavailable"),
        (MessageKey::TrustApprove, "trust_approve"),
        (MessageKey::TrustRecorded, "trust_recorded"),
        (
            MessageKey::TrustSuggestionCommand,
            "trust_suggestion_command",
        ),
        (MessageKey::TrustSuggestionData, "trust_suggestion_data"),
    ]
}

#[test]
fn catalog_presentation_migrated_snapshots_cover_english_neighbours() {
    let values = [
        ("path", "/synthetic/équipe/{literal}"),
        ("error", "English downstream detail {literal}"),
        ("rule", "/core/ @synthetic"),
        ("count", "2"),
        ("digest", "sha256:synthetic"),
        (
            "instruction",
            "maestro trust add /synthetic --confirm-path /synthetic",
        ),
    ];
    let golden: Value =
        serde_json::from_str(include_str!("languages/migrated.golden.json")).unwrap();
    let inventory: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/catalog/presentation/english-neighbours.json"
    ))
    .unwrap();
    for (language, _) in BUILT_INS {
        let interface = Interface::select(language).unwrap();
        let mut rendered = BTreeMap::new();
        for (key, name) in migrated_keys() {
            let template = interface.template(key);
            let arguments: Vec<_> = values
                .iter()
                .copied()
                .filter(|(name, _)| template.contains(&format!("{{{name}}}")))
                .collect();
            rendered.insert(name, interpolate(template, &arguments).unwrap());
        }
        assert_eq!(serde_json::to_value(&rendered).unwrap(), golden[language]);
        for entry in inventory.as_array().unwrap() {
            assert!(rendered.contains_key(entry["key"].as_str().unwrap()));
        }
    }
}

/// Normalize only Rust formatting syntax, never prose from the frozen base.
fn english_template(entry: &Value) -> String {
    let key = entry["key"].as_str().unwrap();
    let mut text = entry["literal"].as_str().unwrap().replace("\\n", "\n");
    for (old, new) in [
        ("{CODEOWNERS_PATH}", "{path}"),
        ("{suggestion}", "{instruction}"),
        ("{failure}", "{error}"),
        ("{quoted}", "{path}"),
        ("{:?}", "{path}"),
    ] {
        text = text.replace(old, new);
    }
    let name = match key {
        "catalog_check_passed" => "count",
        "catalog_compiled" => "digest",
        "diagnostic_instruction" => "instruction",
        _ => "path",
    };
    text.replace("{}", &format!("{{{name}}}"))
}

#[test]
fn catalog_presentation_english_templates_are_byte_identical_to_frozen_neighbours() {
    let inventory: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/catalog/presentation/english-neighbours.json"
    ))
    .unwrap();
    let english: Value = serde_json::from_str(BUILT_INS[0].1).unwrap();
    let mut trust_data = String::new();
    for entry in inventory.as_array().unwrap() {
        let key = entry["key"].as_str().unwrap();
        let template = english_template(entry);
        if key == "trust_suggestion_data" {
            trust_data.push_str(&template);
        } else {
            assert_eq!(
                english[key].as_str().unwrap(),
                template,
                "{}",
                entry["path"]
            );
        }
    }
    assert_eq!(
        english["trust_suggestion_data"].as_str().unwrap(),
        trust_data
    );
}
