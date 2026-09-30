use super::support::{MemoryTree, assert_refused, check_under};
use crate::limits::Limits;

fn with_model_card() -> MemoryTree {
    MemoryTree::valid().with(
        "model-cards/synthetic.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/model-cards/valid.toml"
        )),
    )
}

#[test]
fn valid_model_card_passes_with_kernel_identity_hook() {
    let catalog = check_under(&with_model_card(), &Limits::PRODUCTION).unwrap();
    assert!(
        catalog
            .resources
            .iter()
            .any(|resource| resource.id.kind == "model-card")
    );
}

#[test]
fn invalid_nested_identity_is_refused_by_the_kernel_hook() {
    let valid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog/model-cards/valid.toml"
    ));
    let invalid = valid.replace(
        "router_entry = \"embed\"",
        "router_entry = \"embed\"\nunknown_identity_key = true",
    );
    let unsupported = valid.replace("role = \"embedder\"", "role = \"query_expander\"");
    assert_refused(vec![
        (
            "unknown nested model-card identity key",
            MemoryTree::valid().with("model-cards/synthetic.toml", &invalid),
            "identity: unknown field `unknown_identity_key`",
        ),
        (
            "unsupported role",
            MemoryTree::valid().with("model-cards/synthetic.toml", &unsupported),
            "unknown variant `query_expander`",
        ),
    ]);
}

#[test]
fn undeclared_neighbor_to_delegated_identity_still_refuses() {
    let valid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog/model-cards/valid.toml"
    ));
    let invalid = valid.replace("version = \"2\"", "version = \"2\"\nextra = true");
    assert_refused(vec![(
        "undeclared top-level neighbor",
        MemoryTree::valid().with("model-cards/synthetic.toml", &invalid),
        "extra: unknown key",
    )]);
}
