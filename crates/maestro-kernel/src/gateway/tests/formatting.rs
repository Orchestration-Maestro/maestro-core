//! Model-card document and query formatting.

use super::super::{
    ModelCard,
    card_v2::{Capability, TextFormat},
};
use super::{fixture::Scratch, v2::card_identity};

#[test]
fn supported_document_and_query_formats_are_applied_once() {
    let mut identity = card_identity();
    identity.formats.document = Capability::Supported(TextFormat {
        prefix: "document: ".to_owned(),
        suffix: " <end>".to_owned(),
    });
    identity.formats.query = Capability::Supported(TextFormat {
        prefix: "query: ".to_owned(),
        suffix: " ?".to_owned(),
    });
    let scratch = Scratch::new();
    let card = ModelCard::record_v2(&scratch.store(), &identity).unwrap();

    assert_eq!(card.format_document("body"), "document: body <end>");
    assert_eq!(card.format_query("body"), "query: body ?");
}

#[test]
fn inapplicable_and_legacy_formats_preserve_raw_inputs() {
    let identity = super::v2::answerer_identity();
    let scratch = Scratch::new();
    let card = ModelCard::record_v2(&scratch.store(), &identity).unwrap();
    assert_eq!(card.format_document("document"), "document");
    assert_eq!(card.format_query("query"), "query");

    let legacy = super::fixture::card(super::super::Role::Embedder);
    assert_eq!(legacy.format_document("document"), "document");
    assert_eq!(legacy.format_query("query"), "query");
}
