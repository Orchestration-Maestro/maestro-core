//! Opaque JSON has sorted keys; typed records retain declared field order.

#![cfg(test)]

use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::artifact::Digest;
use serde_json::json;

/// A document with one reverse-ordered opaque subtree.
fn document_at(pointer: &str) -> CanonicalDocument {
    let document = canonicalize(CanonicalizeInput::new("Body\n", "doc.md")).unwrap();
    let mut value = serde_json::to_value(document).unwrap();
    value["extractor_blocks"] = json!([{
        "extractor_id": "synthetic", "markdown_spans": [],
        "original_locations": [{"source_reference": null, "page": null, "locator": null}],
        "structured_content": null,
    }]);
    *value.pointer_mut(pointer).unwrap() =
        serde_json::from_str(r#"{"z":[{"z":2,"a":1},3,1],"a":0}"#).unwrap();
    serde_json::from_value(value).unwrap()
}

#[test]
fn each_opaque_document_field_keeps_pre_cedar_bytes() {
    for (pointer, expected) in [
        (
            "/source_metadata/extraction",
            "68a1a3d2359dab7c6c09f56761c7fcec7c208c5ea2fe307d67e68b3998a10e8b",
        ),
        (
            "/source_metadata/access_policy",
            "8868a484ca87a254c280ae7059014b99cc42f5eb806967cabc7bda48d4982186",
        ),
        (
            "/source_metadata/extra",
            "68ba183453d8dceab1802f4e19129fc651a4f7c67f0ee9a3a7b3d151389e1849",
        ),
        (
            "/input_metadata/extraction",
            "7527f16a03aeefa554b0378b014e9ce2f5c68a71d35b8cdd0c51d555afff5fe5",
        ),
        (
            "/input_metadata/access_policy",
            "7d7f1aab5d5c6d840880d005eff5f6a4e4a209f896090cfcabac9fe9bd30764e",
        ),
        (
            "/input_metadata/extra",
            "a26cde6a10e4b512caed8d61696ae94a98e63796a525a8a9f29579c8101a598c",
        ),
        (
            "/operational_metadata",
            "b6ec55511f18522c63e97606a7b5bb25ed768b3059e63ed920767fabe93e5863",
        ),
        (
            "/access_policy",
            "c16b91a133231ccf9c64c1c80dc3a83335473c6983ca051fbb855b7ea242efc9",
        ),
        (
            "/extractor_blocks/0/original_locations/0/locator",
            "e495b6a618c1cb65156fa87130f684c33591aab43562ae5fce608af3b1e836ff",
        ),
        (
            "/extractor_blocks/0/structured_content",
            "89fbf5333417329a46597628274c2f2d75a882418def717221ed735d5ffb35e4",
        ),
    ] {
        let bytes = serde_json::to_vec_pretty(&document_at(pointer)).unwrap();
        let hash = Digest::of(&bytes);
        assert_eq!(hash.as_str(), expected, "{pointer}");
        assert!(bytes.starts_with(b"{\n  \"schema_version\":"));
    }
}
