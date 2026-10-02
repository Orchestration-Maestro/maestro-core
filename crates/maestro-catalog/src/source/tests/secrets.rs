//! Descriptor-driven secret fields share one strict reference shape, never a resolver.

use super::{
    registry::glossary,
    support::{MemoryTree, check_by},
};
use crate::{
    files::{FileInput, FilePlan, apply},
    limits::Limits,
    source::{Field, FieldType, Known, Registry, SecretReference, build, builtin, frozen_rows},
};
use maestro_test_scratch::scratch_directory;
use std::{collections::BTreeSet, env, fs, path::Path, process::Command};

/// An environment name used only by this test's child process.
const VARIABLE: &str = "MAESTRO_C60_SYNTHETIC_REFERENCE";
/// Distinct synthetic bytes that must never leave the child's environment.
const SENTINEL: &str = "c60-synthetic-resolved-value-never-output";
/// Metadata for a checked, owned fixture.
const METADATA: &str = "[metadata]\nschema = \"maestro-source/2\"\nmaturity = \"reviewed\"\n\
    rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\n";

/// Settings, backends and extensions select secret-bearing fields as descriptor data.
fn registry() -> Registry {
    let mut registry = builtin().unwrap();
    for kind in ["settings", "backend", "extension"] {
        let mut descriptor = glossary();
        descriptor.kind = kind.to_owned();
        descriptor.directory = format!("synthetic-{kind}");
        descriptor.name_field = None;
        descriptor.fields = vec![Field::required(
            "credential",
            serde_json::from_str::<FieldType>("\"secret-reference\"").unwrap(),
        )];
        registry.register(descriptor).unwrap();
    }
    registry
}

/// A common-area resource with a secret field, using the real source parser.
fn tree(kind: &str, value: &str) -> MemoryTree {
    MemoryTree::owned().with(
        &format!("synthetic-{kind}/example.toml"),
        &format!("credential = {value}\n{METADATA}"),
    )
}

#[test]
fn secret_reference_oneof_accepts() {
    let registry = registry();
    for kind in ["settings", "backend", "extension"] {
        for reference in [
            "{ env = \"MAESTRO_C60_SYNTHETIC_REFERENCE\" }",
            "{ keychain = { service = \"synthetic-review\", account = \"token\" } }",
        ] {
            let catalog = check_by(&tree(kind, reference), &registry, &Limits::PRODUCTION).unwrap();
            let resource = catalog
                .resources
                .iter()
                .find(|resource| resource.id.kind == kind)
                .unwrap();
            let encoded = serde_json::to_string(&resource.fields["credential"]).unwrap();
            let typed = resource.fields["credential"]
                .decode::<SecretReference>()
                .unwrap();
            assert_eq!(
                serde_json::to_value(&typed).unwrap(),
                serde_json::to_value(&resource.fields["credential"]).unwrap()
            );
            assert!(!encoded.contains(SENTINEL));
            assert!(encoded.contains("env") || encoded.contains("keychain"));
        }
        let both = "{ env = \"NAME\", keychain = { service = \"review\", account = \"token\" } }";
        assert!(
            check_by(&tree(kind, both), &registry, &Limits::PRODUCTION).is_err(),
            "both forms in {kind}"
        );
    }
}

#[test]
fn secret_literal_channels_refuse() {
    let registry = registry();
    for kind in ["settings", "backend", "extension"] {
        for value in [
            "\"synthetic-literal\"",
            "\"https://user:pass@host\"",
            "\"https://token@host\"",
            "{ env = { NAME = \"synthetic-literal\" } }",
            "{ NAME = \"synthetic-literal\" }",
            "{ args = [\"--token\", \"synthetic-literal\"] }",
            "{ default = \"synthetic-literal\" }",
            "{ env = \"NAME\", default = \"synthetic-literal\" }",
            "{ env = \"NAME\", args = [\"synthetic-literal\"] }",
            "{}",
            "{ env = \" \" }",
            "{ env = 1 }",
            "{ keychain = { service = \"review\" } }",
            "{ keychain = { service = \"\", account = \"token\" } }",
            "{ keychain = { service = \"review\", account = \" \" } }",
            "{ keychain = { service = \"review\", account = \"token\", value = \"literal\" } }",
        ] {
            let refusal = check_by(&tree(kind, value), &registry, &Limits::PRODUCTION).unwrap_err();
            assert!(
                refusal
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.key == "credential"),
                "{kind}: {value}: {refusal}"
            );
            assert!(
                !refusal.to_string().contains("synthetic-literal"),
                "diagnostics disclose input"
            );
        }
    }
}

#[test]
fn check_install_explain_never_resolve_secrets() {
    if env::var(VARIABLE).as_deref() != Ok(SENTINEL) {
        // A child environment avoids mutating process-global state in parallel tests.
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "source::tests::secrets::check_install_explain_never_resolve_secrets",
                "--nocapture",
            ])
            .env(VARIABLE, SENTINEL)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains(SENTINEL));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(SENTINEL));
        return;
    }
    // C18 adds the explain consumer and extends this test; no explain API exists yet.
    let registry = registry();
    let rows = frozen_rows();
    let settings = BTreeSet::<String>::new();
    let scratch = scratch_directory().unwrap();
    for kind in ["settings", "backend", "extension"] {
        let reference = format!("{{ env = \"{VARIABLE}\" }}");
        let tree = tree(kind, &reference);
        let checked = check_by(&tree, &registry, &Limits::PRODUCTION).unwrap();
        let (built, _) = build(
            &tree,
            &registry,
            &Limits::PRODUCTION,
            Known {
                rows: &rows,
                settings: &settings,
            },
        )
        .unwrap();
        assert_eq!(checked, built);
        let resource = checked
            .resources
            .iter()
            .find(|resource| resource.id.kind == kind)
            .unwrap();
        let bytes = serde_json::to_vec(&resource.fields).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains(SENTINEL));
        assert!(String::from_utf8_lossy(&bytes).contains(VARIABLE));
        assert!(!format!("{checked:?}\n{built:?}").contains(SENTINEL));
        let plan = FilePlan::preview(
            &scratch,
            [
                FileInput::new(format!("{kind}.json"), bytes.clone()),
                FileInput::new(format!("{kind}.lock"), bytes),
            ],
        )
        .unwrap();
        assert!(!serde_json::to_string(&plan).unwrap().contains(SENTINEL));
        apply(&scratch, &plan).unwrap();
    }
    assert_no_sentinel(&scratch);
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn c60_review_probe_sequence_shapes_refuse() {
    let registry = registry();
    let mut accepted = Vec::new();
    for kind in ["settings", "backend", "extension"] {
        for value in [
            "[\"NAME\"]",
            "[[\"review\", \"token\"]]",
            "[{ service = \"review\", account = \"token\" }]",
            "{ keychain = [\"review\", \"token\"] }",
            "[{ env = \"NAME\" }]",
        ] {
            let result = check_by(&tree(kind, value), &registry, &Limits::PRODUCTION);
            println!("sequence probe {kind} {value}: {result:?}");
            if result.is_ok() {
                accepted.push(format!("{kind}: {value}"));
            }
        }
    }
    for value in [
        "[\"NAME\"]",
        "[[\"review\",\"token\"]]",
        "[{\"service\":\"review\",\"account\":\"token\"}]",
        "{\"keychain\":[\"review\",\"token\"]}",
        "[{\"env\":\"NAME\"}]",
    ] {
        let result = serde_json::from_str::<SecretReference>(value);
        println!("public decoder {value}: {result:?}");
        if result.is_ok() {
            accepted.push(format!("JSON: {value}"));
        }
    }
    assert!(
        accepted.is_empty(),
        "non-map secret shapes accepted: {accepted:?}"
    );
}

#[test]
fn c60_review_probe_nested_secret_fields() {
    let mut registry = builtin().unwrap();
    let mut descriptor = glossary();
    descriptor.kind = "nested-secret".to_owned();
    descriptor.directory = "synthetic-nested-secret".to_owned();
    descriptor.fields = vec![Field::required(
        "outer",
        FieldType::Table {
            fields: vec![Field::required("credential", FieldType::SecretReference)],
        },
    )];
    registry.register(descriptor).unwrap();
    for (value, accepts) in [
        ("{ env = \"NAME\" }", true),
        (
            "{ keychain = { service = \"review\", account = \"token\" } }",
            true,
        ),
        ("\"synthetic-literal\"", false),
        (
            "{ env = \"NAME\", keychain = { service = \"review\", account = \"token\" } }",
            false,
        ),
        ("{ default = \"synthetic-literal\" }", false),
    ] {
        for inline in [true, false] {
            let source = if inline {
                format!("outer = {{ credential = {value} }}\n{METADATA}")
            } else {
                format!("{METADATA}[outer]\ncredential = {value}\n")
            };
            let tree = MemoryTree::owned().with("synthetic-nested-secret/example.toml", &source);
            let result = check_by(&tree, &registry, &Limits::PRODUCTION);
            assert_eq!(
                result.is_ok(),
                accepts,
                "inline={inline}: {value}: {result:?}"
            );
            if let Err(refusal) = result {
                assert!(
                    refusal
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.key == "outer.credential")
                );
                assert!(!refusal.to_string().contains("synthetic-literal"));
            }
        }
    }
}

/// Inspect every installed file and real durable ownership receipt, not just projections.
fn assert_no_sentinel(path: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            assert_no_sentinel(&path);
        } else {
            assert!(!String::from_utf8_lossy(&fs::read(path).unwrap()).contains(SENTINEL));
        }
    }
}
