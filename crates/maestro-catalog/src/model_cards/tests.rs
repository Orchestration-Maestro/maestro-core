use super::{declaration::Declaration, register::register};
use maestro_kernel::{
    document::Collection,
    gateway::{ModelCard, Role},
    scope::{Right, Scope},
    store::Database,
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

const VALID: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/catalog/model-cards/valid.toml"
));
const INVALID: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/catalog/model-cards/invalid.toml"
));

#[test]
fn declaration_preserves_the_kernel_identity_and_declares_version_separately() {
    let declaration: Declaration = toml::from_str(VALID).unwrap();
    assert_eq!(declaration.version, "2");
    assert!(!declaration.metadata.contains_key("version"));
    let card = ModelCard::from_identity(&declaration.identity).unwrap();
    assert_eq!(card.identity(), Some(&declaration.identity));
}

#[test]
fn kernel_identity_refuses_an_unknown_nested_field_and_role() {
    let unknown = VALID.replace(
        "router_entry = \"embed\"",
        "router_entry = \"embed\"\nunknown_identity_key = true",
    );
    assert!(toml::from_str::<Declaration>(&unknown).is_err());

    let unsupported = VALID.replace("role = \"embedder\"", "role = \"query_expander\"");
    let error = toml::from_str::<Declaration>(&unsupported)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unknown variant `query_expander`"),
        "{error}"
    );
}

#[test]
fn invalid_fixture_is_refused_by_kernel_identity_deserialization() {
    assert!(toml::from_str::<Declaration>(INVALID).is_err());
}

#[test]
fn same_card_registration_returns_the_kernel_record_without_selection_changes() {
    let mut declaration: Declaration = toml::from_str(VALID).unwrap();
    let root = test_root();
    fs::create_dir_all(&root).unwrap();
    let database = Database::open(&root.join("kernel.sqlite3"), &root.join("artifacts")).unwrap();
    database
        .record_collection(&Collection {
            id: "synthetic".to_owned(),
            title: "synthetic".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .grant(
            "catalog-test",
            &"workspace/default/collection/synthetic"
                .parse::<Scope>()
                .unwrap(),
            Right::Read,
            "test",
        )
        .unwrap();
    let scopes = database.visible("catalog-test").unwrap();
    assert!(register(&database, &scopes, "synthetic", &declaration).is_err());
    assert!(
        database
            .model_cards(&scopes, "synthetic", Role::Embedder)
            .unwrap()
            .is_empty()
    );

    let evidence = database
        .put(b"synthetic qualification", "application/octet-stream")
        .unwrap();
    declaration.identity.formats.qualification_digest = evidence.clone();
    declaration
        .identity
        .provenance
        .artifacts
        .insert("native-qualification".to_owned(), evidence);

    let first = register(&database, &scopes, "synthetic", &declaration).unwrap();
    let repeated = register(&database, &scopes, "synthetic", &declaration).unwrap();
    assert_eq!(first.id, repeated.id);
    assert_eq!(first.digest, repeated.digest);
    assert!(
        database
            .selected_model_card(&scopes, "synthetic", Role::Embedder)
            .unwrap()
            .is_none()
    );
    fs::remove_dir_all(root).unwrap();
}

fn test_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!("maestro-card-test-{nonce}"))
}

#[test]
fn explicit_registration_refuses_unreviewed_and_unauthorized_declarations() {
    let authored = VALID.replace("maturity = \"reviewed\"", "maturity = \"authored\"");
    let declaration: Declaration = toml::from_str(&authored).unwrap();
    let root = env::temp_dir().join(format!("maestro-card-test-{}", process::id()));
    drop(fs::remove_dir_all(&root));
    fs::create_dir_all(&root).unwrap();
    let database = Database::open(&root.join("kernel.sqlite3"), &root.join("artifacts")).unwrap();
    database
        .record_collection(&Collection {
            id: "synthetic".to_owned(),
            title: "synthetic".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();

    let scopes = database.visible("unauthorized").unwrap();
    assert_eq!(
        register(&database, &scopes, "synthetic", &declaration),
        Err("model-card registration requires reviewed maturity".to_owned())
    );
    let reviewed: Declaration = toml::from_str(VALID).unwrap();
    let refused = register(&database, &scopes, "synthetic", &reviewed).unwrap_err();
    assert_eq!(refused, "the collection is not writable in this scope");
    fs::remove_dir_all(root).unwrap();
}
