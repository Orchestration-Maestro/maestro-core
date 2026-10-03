//! Decoder ceilings are independent of later capability admission.
use super::{
    n30_support::Fixture,
    n57_processing_artifacts::{check, initial},
};
use maestro_acquisition::{
    adaptation::{
        artifacts::{DedupKeys, S1ChunkStrategy},
        snapshot::ProcessingSnapshot,
    },
    policy::resolve::parse_resource,
};
use serde_json::{Map, Value, json};

#[test]
fn n57_effective_inventories_bound_counts_and_names() {
    let fixture = Fixture::new();
    let mut value = json!(initial(&fixture));
    let entry = value["effective"]["approved_cleanup"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    for size in [1000, 1001] {
        let entries = (0..size)
            .map(|index| (format!("item-{index}"), entry.clone()))
            .collect::<Map<_, _>>();
        value["effective"]["approved_cleanup"] = json!(entries);
        assert_eq!(
            parse_resource::<ProcessingSnapshot>(&serde_json::to_vec(&value).unwrap()).is_ok(),
            size == 1000
        );
    }
    value["effective"]["approved_cleanup"] = json!({"../bad":entry});
    assert!(parse_resource::<ProcessingSnapshot>(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn n57_key_and_text_declaration_bounds_precede_capability_checks() {
    let mut keys: Value =
        serde_json::from_slice(include_bytes!("../fixtures/adaptation/dedup.json")).unwrap();
    for size in [1000, 1001] {
        keys["exact"] = json!(vec!["original_digest"; size]);
        assert_eq!(
            parse_resource::<DedupKeys>(&serde_json::to_vec(&keys).unwrap()).is_ok(),
            size == 1000
        );
    }
    let mut strategy: Value =
        serde_json::from_slice(include_bytes!("../fixtures/adaptation/chunk.json")).unwrap();
    for text in ["x".repeat(4096), "x".repeat(4097), "null\0text".into()] {
        strategy["chunker_version"] = json!(text);
        assert_eq!(
            parse_resource::<S1ChunkStrategy>(&serde_json::to_vec(&strategy).unwrap()).is_ok(),
            text.len() == 4096
        );
    }
}

#[test]
fn n57_exact_source_and_profile_inventories_are_required() {
    let fixture = Fixture::new();
    let old = initial(&fixture);
    assert!(check(&fixture, &old));
    for fault in ["extra-source", "extra-profile", "missing-profile"] {
        let mut snapshot = old.clone();
        match fault {
            "extra-source" => {
                let source = snapshot.effective.sources.get("notes").unwrap().clone();
                snapshot.effective.sources.insert("extra".into(), source);
            }
            "extra-profile" => {
                let profile = snapshot.effective.profiles.get("novel").unwrap().clone();
                snapshot.effective.profiles.insert("extra".into(), profile);
            }
            _ => {
                snapshot.effective.profiles.remove("novel");
            }
        }
        assert!(!check(&fixture, &snapshot), "{fault}");
    }
}

#[test]
fn n57_review_dedup_keys_are_strings_not_unit_variant_objects() {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../fixtures/adaptation/dedup.json")).unwrap();
    let original = parse_resource::<DedupKeys>(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(original.validate().is_ok());
    value["exact"][0] = json!({"original_digest":null});
    let decoded = parse_resource::<DedupKeys>(&serde_json::to_vec(&value).unwrap());
    println!(
        "R2 object_alias_parses={} executable_tuple={}",
        decoded.is_ok(),
        decoded.as_ref().is_ok_and(|value| value.validate().is_ok())
    );
    assert!(decoded.is_err(), "dedup key must be a snake-case string");
}
