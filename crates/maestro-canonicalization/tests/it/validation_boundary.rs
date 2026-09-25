//! Regression checks for review findings at the source and JSON trust boundaries.
#![cfg(test)]
use maestro_canonicalization::{
    CanonicalizeInput, Severity, ValidationStatus, canonicalize, save_document, validate_document,
};
use serde_json::json;
use std::{env, process};

#[test]
fn duplicate_yaml_keys_fail_including_nested_permissions() {
    for md in [
        "---\ntitle: first\ntitle: second\n---\nBody\n",
        "---\naccess_policy:\n  classification: private\n  classification: public\n---\nBody\n",
    ] {
        let doc = canonicalize(CanonicalizeInput::new(md, "doc.md")).unwrap();
        assert_eq!(doc.validation_status, ValidationStatus::Failed);
        assert!(doc.warnings.iter().any(|w| w.code == "invalid_frontmatter"));
    }
}

#[test]
fn compatible_extraction_metadata_preserves_extra_details() {
    let md =
        "---\nconverter: docling\nextraction:\n  converter: docling\n  version: '2.0'\n---\nBody\n";
    let doc = canonicalize(CanonicalizeInput::new(md, "doc.md")).unwrap();
    assert_ne!(doc.validation_status, ValidationStatus::Failed);
    assert_eq!(
        doc.source_metadata.extraction,
        Some(json!({"converter":"docling","version":"2.0"}))
    );
    let conflict = md.replace(
        "converter: docling\n  version",
        "converter: different\n  version",
    );
    assert_eq!(
        canonicalize(CanonicalizeInput::new(&conflict, "doc.md"))
            .unwrap()
            .validation_status,
        ValidationStatus::Failed
    );
}

#[test]
fn nested_duplicate_references_cannot_disappear_silently() {
    let md = "> [x]: https://first.test\n> [x]: https://second.test\n>\n> [go][x]\n";
    let doc = canonicalize(CanonicalizeInput::new(md, "doc.md")).unwrap();
    assert_ne!(doc.validation_status, ValidationStatus::Failed);
    let raw = doc
        .blocks
        .iter()
        .find(|block| {
            block.block_type == maestro_canonicalization::BlockType::Raw
                && block.retrieval_text.contains("https://second.test")
        })
        .unwrap();
    assert_eq!(
        raw.retrieval_text,
        &md[raw.source_spans[0].start..raw.source_spans[0].end]
    );
    assert_eq!(doc.links[0].destination, "https://first.test");
    assert!(
        doc.warnings
            .iter()
            .any(|w| { w.code == "source_fallback" && w.block_id.as_ref() == Some(&raw.block_id) })
    );
}

#[test]
fn recognized_container_markers_are_not_false_content_loss() {
    for md in [
        "> [!NOTE]\n> Keep this warning.\n",
        "Term\n: Description\n",
        "Text[^a].\n\n[^a]: Body.\n",
    ] {
        let doc = canonicalize(CanonicalizeInput::new(md, "doc.md")).unwrap();
        assert_ne!(
            doc.validation_status,
            ValidationStatus::Failed,
            "{md}: {:?}",
            doc.warnings
        );
    }
}

#[test]
fn altered_json_is_rejected_by_replay_validation() {
    let md = "Do not [restart](https://example.test).\n";
    let original = canonicalize(CanonicalizeInput::new(md, "doc.md")).unwrap();
    assert!(
        !validate_document(&original, md)
            .iter()
            .any(|w| w.severity == Severity::Error)
    );
    for change in 0..7 {
        let mut doc = original.clone();
        match change {
            0 => doc.blocks[0].retrieval_text = "Restart immediately.".into(),
            1 => doc.access_policy = Some(json!("public")),
            2 => doc.source_reference = Some("fabricated.pdf".into()),
            3 => {
                doc.revision_id = "changed".into();
                doc.blocks[0].revision_id = "changed".into();
            }
            4 => doc.links.clear(),
            5 => doc.blocks[0].source_spans[0].end = 2,
            _ => doc.warnings.clear(),
        }
        assert!(
            validate_document(&doc, md)
                .iter()
                .any(|w| w.severity == Severity::Error),
            "mutation {change}"
        );
        let path = env::temp_dir().join(format!("canonical-refuse-{}-{change}", process::id()));
        assert!(save_document(&doc, md, &path).is_err(), "mutation {change}");
        assert!(!path.exists());
    }
}
