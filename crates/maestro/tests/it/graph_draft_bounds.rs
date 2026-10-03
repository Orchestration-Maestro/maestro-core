//! Private drafting file bounds and receipt schema regressions.
use super::{
    graph_draft::{Router, answer, authority, draft, manifest},
    support::Home,
};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::fs;

#[test]
fn graph_draft_refuses_oversized_manifest() {
    oversized_input("manifest");
}
#[test]
fn graph_draft_refuses_oversized_inventory() {
    oversized_input("inventory");
}
#[test]
fn graph_draft_refuses_oversized_prompt() {
    oversized_input("prompt");
}

/// Each oversized file must fail during admission, before opening the journal.
fn oversized_input(kind: &str) {
    let home = Home::bare();
    let (card, generation) = authority(&home);
    let router = Router::new(answer());
    let path = manifest(&home, &router.url, &card, generation);
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let private = path.parent().unwrap();
    match kind {
        "manifest" => {
            let mut text = value.to_string();
            text.push_str(&" ".repeat(1024 * 1024));
            fs::write(&path, text).unwrap();
        }
        "inventory" => {
            let input = private.join("inventory.json");
            let mut text = fs::read_to_string(&input).unwrap();
            text.push_str(&" ".repeat(16 * 1024 * 1024));
            fs::write(input, &text).unwrap();
            value["inventory_digest"] = json!(Digest::of(text.as_bytes()));
            fs::write(&path, value.to_string()).unwrap();
        }
        "prompt" => {
            let text = "x".repeat(4097);
            fs::write(private.join("prompt.txt"), &text).unwrap();
            value["prompt_digest"] = json!(Digest::of(text.as_bytes()));
            fs::write(&path, value.to_string()).unwrap();
        }
        _ => panic!("unknown file defect"),
    }
    let output = draft(&home, &path);
    assert_eq!(output.status.code(), Some(2), "{kind}: {output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("graph_manifest_invalid"));
    assert!(!private.join("draft.lock").exists());
}

#[test]
fn graph_draft_replay_refuses_unknown_missing_and_oversized_receipt_envelopes() {
    for defect in ["unknown", "missing", "oversized"] {
        let home = Home::bare();
        let (card, generation) = authority(&home);
        let router = Router::new(answer());
        let path = manifest(&home, &router.url, &card, generation);
        assert!(draft(&home, &path).status.success());
        let receipt = path
            .parent()
            .unwrap()
            .join("draft-receipt-00000000000000000000.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
        match defect {
            "unknown" => value["schema"] = json!("maestro-graph-draft-receipt/999"),
            "missing" => {
                value.as_object_mut().unwrap().remove("schema");
            }
            "oversized" => (),
            _ => panic!("unknown file defect"),
        }
        let mut text = value.to_string();
        if defect == "oversized" {
            text.push_str(&" ".repeat(16 * 1024 * 1024));
        }
        fs::write(receipt, text).unwrap();
        assert!(!draft(&home, &path).status.success(), "{defect}");
    }
}
