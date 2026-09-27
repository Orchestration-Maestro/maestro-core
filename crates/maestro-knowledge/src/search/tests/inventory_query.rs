//! Exact English and French inventory-query grammar.

use super::super::inventory_query::{InventoryParseError, inventory_request};
use crate::query::{QueryKind, understand};
use maestro_kernel::retrieval::InventoryRequest;

#[test]
fn accepts_only_the_six_document_set_and_version_forms() {
    for (text, expected) in [
        (
            "how many documents",
            InventoryRequest::DocumentsBySet { set: None },
        ),
        (
            "COMBIEN DE DOCUMENTS?",
            InventoryRequest::DocumentsBySet { set: None },
        ),
        (
            "list all document sets",
            InventoryRequest::DocumentsBySet { set: None },
        ),
        (
            "liste des lots de documents",
            InventoryRequest::DocumentsBySet { set: None },
        ),
        (
            "list all versions",
            InventoryRequest::Versions { set: None },
        ),
        (
            "LÍSTE DES VERSIONS?",
            InventoryRequest::Versions { set: None },
        ),
    ] {
        assert_eq!(
            inventory_request(&understand(text)).unwrap(),
            Some(expected),
            "{text}"
        );
    }
}

#[test]
fn parses_one_json_set_value_without_normalizing_its_contents() {
    for text in [
        r#"how many documents in set "Guide \"A\"/FR"?"#,
        r#"combien de documents dans le lot "Guide \"A\"/FR""#,
        r#"list all versions in set "Guide \"A\"/FR""#,
        r#"liste des versions dans le lot "Guide \"A\"/FR""#,
    ] {
        let request = inventory_request(&understand(text)).unwrap().unwrap();
        match request {
            InventoryRequest::DocumentsBySet { set } | InventoryRequest::Versions { set } => {
                assert_eq!(set.as_deref(), Some("Guide \"A\"/FR"));
            }
        }
    }
}

#[test]
fn json_set_words_do_not_change_inventory_classification() {
    let text = r#"how many documents in set "Error FR-2024 vs 3.0""#;
    let understood = understand(text);
    assert_eq!(understood.kind, QueryKind::Global);
    assert_eq!(understood.version.as_deref(), Some("3.0"));
    assert!(understood.identifiers.iter().any(|id| id.text == "FR-2024"));
    assert_eq!(
        inventory_request(&understood).unwrap(),
        Some(InventoryRequest::DocumentsBySet {
            set: Some("Error FR-2024 vs 3.0".to_owned())
        })
    );
}

#[test]
fn malformed_quoted_set_words_do_not_override_global_classification() {
    let understood = understand(r#"how many documents in set"Error""#);
    assert_eq!(understood.kind, QueryKind::Global);
    assert_eq!(inventory_request(&understood).unwrap(), None);
}

#[test]
fn rejects_malformed_filters_and_does_not_guess_other_global_questions() {
    for text in [
        r#"how many documents in set """#,
        "how many documents in set guide",
        r#"list all versions in set "guide" trailing"#,
    ] {
        assert!(inventory_request(&understand(text)).is_err(), "{text}");
    }
    assert_eq!(
        InventoryParseError.to_string(),
        "inventory set must be one nonempty JSON string"
    );
    assert_eq!(
        understand("how many documents about topics").kind,
        QueryKind::Global
    );
    assert_eq!(
        inventory_request(&understand("how many documents about topics")).unwrap(),
        None
    );
    assert_eq!(
        inventory_request(&understand("compare all versions")).unwrap(),
        None
    );
}
