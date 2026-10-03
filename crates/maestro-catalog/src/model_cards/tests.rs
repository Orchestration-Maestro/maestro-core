//! Tests for checked model-card declarations and scoped registration.

use super::{declaration::Declaration, register::register};
use crate::source::Maturity;
use crate::source::tests::support::checked_model_card;
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

const KERNEL_V2_GOLDEN_DIGEST: &str =
    "fbaa5c760ee799f3ddaf4d7c07ff34b431824544f7a7595084d4ce8e3f346323";

const VALID: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/catalog/model-cards/valid.toml"
));
#[test]
fn declaration_preserves_the_kernel_identity_and_declares_version_separately() {
    let declaration = Declaration::from_resource(&checked_model_card(VALID)).unwrap();
    assert_eq!(declaration.version, "2");
    assert_eq!(declaration.maturity, Maturity::Reviewed);
    let card = ModelCard::from_identity(&declaration.identity).unwrap();
    assert_eq!(card.identity(), Some(&declaration.identity));
    assert_eq!(
        format!("{:?}", card.digest()),
        format!("Digest({KERNEL_V2_GOLDEN_DIGEST:?})")
    );
}

#[test]
fn changing_identity_changes_its_kernel_fingerprint() {
    let resource = checked_model_card(VALID);
    let original = Declaration::from_resource(&resource).unwrap();
    let mut changed = original.identity.clone();
    changed.weights.upstream_revision.push('x');
    let original_digest = ModelCard::from_identity(&original.identity)
        .unwrap()
        .digest()
        .clone();
    let changed_digest = ModelCard::from_identity(&changed).unwrap().digest().clone();
    assert_ne!(original_digest, changed_digest);
}

#[test]
fn same_card_registration_returns_the_kernel_record_without_selection_changes() {
    let mut declaration = Declaration::from_resource(&checked_model_card(VALID)).unwrap();
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
    let missing_evidence = register(&database, &scopes, "synthetic", &declaration).unwrap_err();
    assert!(
        missing_evidence.contains("no artifact is recorded under"),
        "{missing_evidence}"
    );
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
    drop(database);
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
    let declaration = Declaration::from_resource(&checked_model_card(&authored)).unwrap();
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
    let reviewed = Declaration::from_resource(&checked_model_card(VALID)).unwrap();
    let refused = register(&database, &scopes, "synthetic", &reviewed).unwrap_err();
    assert_eq!(refused, "the collection is not writable in this scope");
    drop(database);
    fs::remove_dir_all(root).unwrap();
}
