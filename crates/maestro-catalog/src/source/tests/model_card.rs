//! Tests for catalog model-card validation through its kernel hook.

use super::support::{MemoryTree, assert_refused_by, check_by};
use crate::{limits::Limits, source::builtin};

fn with_model_card() -> MemoryTree {
    MemoryTree::default().with(
        "core/llm/models/embedder/synthetic.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/model-cards/valid.toml"
        )),
    )
}

#[test]
fn valid_model_card_passes_with_kernel_identity_hook() {
    let catalog = check_by(&with_model_card(), &builtin().unwrap(), &Limits::PRODUCTION).unwrap();
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
    let machine_path = valid.replace(
        "upstream_model_id = \"org/model\"",
        &format!(
            "upstream_model_id = \"{}\"",
            ["/home", "x", "model.gguf"].join("/")
        ),
    );
    let credentialed_url = valid.replace(
        "source_url = \"https://example.invalid/model\"",
        "source_url = \"https://u:p@example.invalid/m\"",
    );
    assert_refused_by(
        &builtin().unwrap(),
        vec![
            (
                "unknown nested model-card identity key",
                MemoryTree::default().with("core/llm/models/embedder/synthetic.toml", &invalid),
                "identity: unknown field `unknown_identity_key`",
            ),
            (
                "unsupported role",
                MemoryTree::default().with("core/llm/models/embedder/synthetic.toml", &unsupported),
                "unknown variant `query_expander`",
            ),
            (
                "machine path",
                MemoryTree::default()
                    .with("core/llm/models/embedder/synthetic.toml", &machine_path),
                concat!(
                    "identity: not a valid maestro-model-card/1 or maestro-model-card/2 ",
                    "model card: upstream_model_id is a machine path"
                ),
            ),
            (
                "URL credentials",
                MemoryTree::default()
                    .with("core/llm/models/embedder/synthetic.toml", &credentialed_url),
                concat!(
                    "identity: not a valid maestro-model-card/1 or maestro-model-card/2 ",
                    "model card: source_url must be an HTTP(S) URL without credentials"
                ),
            ),
        ],
    );
}

#[test]
fn declaration_version_must_be_two() {
    let valid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog/model-cards/valid.toml"
    ));
    let invalid = valid.replace("version = \"2\"", "version = \"1\"");
    assert_refused_by(
        &builtin().unwrap(),
        vec![(
            "unsupported model-card declaration version",
            MemoryTree::default().with("core/llm/models/embedder/synthetic.toml", &invalid),
            "version: must be \"2\"",
        )],
    );
}

#[test]
fn delegated_identity_non_table_is_refused_by_the_kernel_hook() {
    let valid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog/model-cards/valid.toml"
    ));
    let metadata = valid
        .split("[metadata]")
        .nth(1)
        .unwrap()
        .split("[identity]")
        .next()
        .unwrap();
    let invalid = format!("version = \"2\"\nidentity = \"x\"\n[metadata]{metadata}");
    assert_refused_by(
        &builtin().unwrap(),
        vec![(
            "non-table identity",
            MemoryTree::default().with("core/llm/models/embedder/synthetic.toml", &invalid),
            "identity: invalid type: string \"x\", expected struct CardIdentity",
        )],
    );
}

#[test]
fn undeclared_neighbor_to_delegated_identity_still_refuses() {
    let valid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog/model-cards/valid.toml"
    ));
    let invalid = valid.replace("version = \"2\"", "version = \"2\"\nextra = true");
    assert_refused_by(
        &builtin().unwrap(),
        vec![(
            "undeclared top-level neighbor",
            MemoryTree::default().with("core/llm/models/embedder/synthetic.toml", &invalid),
            "extra: unknown key",
        )],
    );
}
