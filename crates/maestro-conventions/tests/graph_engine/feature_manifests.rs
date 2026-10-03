//! E07a's default/native feature and required-CI ownership contracts.

use maestro_conventions::root;
use std::fs;
use toml::Value;

/// The repository's TOML document at `path`.
fn document(path: &str) -> Value {
    toml::from_str(&fs::read_to_string(root().join(path)).unwrap()).unwrap()
}

#[test]
fn graph_engine_is_optional_and_forwarded_only_by_the_engine_feature() {
    let knowledge = document("crates/maestro-knowledge/Cargo.toml");
    let cli = document("crates/maestro/Cargo.toml");
    for manifest in [&knowledge, &cli] {
        assert!(manifest["features"].get("default").is_none());
    }
    assert_eq!(
        knowledge["dependencies"]["lbug"]["optional"].as_bool(),
        Some(true)
    );
    assert_eq!(
        knowledge["dependencies"]["lbug"]["workspace"].as_bool(),
        Some(true)
    );
    assert_eq!(
        knowledge["features"]["engine"].as_array().unwrap(),
        &[Value::String("dep:lbug".into())]
    );
    assert_eq!(
        cli["features"]["engine"].as_array().unwrap(),
        &[Value::String("maestro-knowledge/engine".into())]
    );
    let workspace = document("Cargo.toml");
    assert_eq!(
        workspace["workspace"]["dependencies"]["lbug"]["default-features"].as_bool(),
        Some(false)
    );
}

#[test]
fn path_only_graph_constructor_is_disallowed() {
    let config = document("clippy.toml");
    assert!(
        config["disallowed-methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|method| { method["path"].as_str() == Some("lbug::Database::new") })
    );
}

#[test]
fn required_ci_owns_native_coverage_and_the_exact_engine_source() {
    let policy = document("maestro-quality.toml");
    assert_eq!(
        policy["ci"]["coverage-features"].as_array().unwrap(),
        &[
            Value::String("maestro/engine".into()),
            Value::String("maestro-knowledge/engine".into()),
        ]
    );
    assert_eq!(
        policy["ci"]["mutation-engine"]["features"]
            .as_array()
            .unwrap(),
        &[Value::String("engine".into())]
    );
    assert_eq!(
        policy["ci"]["mutation-engine"]["files"].as_array().unwrap(),
        &["open", "schema", "transaction", "rows"].map(|name| {
            Value::String(format!(
                "crates/maestro-knowledge/src/graph/projection/engine/{name}.rs"
            ))
        })
    );
    let mutants = document(".cargo/mutants.toml");
    assert!(mutants.get("features").is_none());
    assert!(mutants.get("test_workspace").is_none());
}
