//! Immutable loader record contracts (also run without the native engine).
use super::super::{
    ProjectionSnapshot,
    checkpoint::{Journal, Manifest, batch_count, prefix},
    content::tests::fact,
};
use maestro_kernel::{artifact::Digest, facts::ClaimSetRecord};
use maestro_test_scratch::scratch_directory;
use serde_json::json;
use std::{fs, path::PathBuf};

fn fixture() -> (PathBuf, ProjectionSnapshot, Manifest) {
    let path = scratch_directory().unwrap();
    // Only Unix journals need symlinked temp parents resolved.
    #[cfg(unix)]
    let path = fs::canonicalize(path).unwrap();
    let fact = fact();
    let rows = ProjectionSnapshot {
        scope: fact.scope.clone(),
        claim_set: ClaimSetRecord {
            id: Digest::of(b"set"),
            collection_id: "c".into(),
            claims: vec![fact.claim.clone()],
        },
        resolution_id: Digest::of(b"resolution"),
        resolver_version: "resolver/1".into(),
        history: vec![],
        edges: vec![],
        facts: vec![fact],
    };
    let manifest = serde_json::from_value(json!({
        "schema": "graph-loader/1", "job": "job", "scope": ["c", 1],
        "claim_set": rows.claim_set.id, "pins": ["resolution", "resolver/1", "settings", "lock"],
        "counts": [0, 1], "digest": prefix(&rows, 1).unwrap().content_digest,
    }))
    .unwrap();
    (path, rows, manifest)
}

#[test]
fn checkpoint_count_is_derived_from_both_row_families() {
    for (rows, expected) in [(0, 0), (1, 1), (64, 1), (65, 2), (128, 2)] {
        assert_eq!(batch_count(rows).unwrap(), expected);
    }
    assert!(batch_count(usize::MAX).is_err());
}

#[test]
fn checkpoint_manifest_rejects_unknown_fields_and_unsupported_schema() {
    let bytes = br#"{"schema":"unknown","extra":true}"#;
    assert!(serde_json::from_slice::<Manifest>(bytes).is_err());
}

#[test]
fn checkpoint_publication_is_immutable_and_unknown_or_changed_files_refuse() {
    let (path, rows, manifest) = fixture();
    let journal = Journal::create(&path, manifest).unwrap();
    assert_eq!(journal.ordinals().unwrap(), 0);
    journal.record(&rows, 1).unwrap();
    assert_eq!(journal.ordinals().unwrap(), 1);
    journal.validate_checkpoint(&rows, 1).unwrap();
    let bytes = fs::read(path.join("loader/0000000001.json")).unwrap();
    assert!(journal.record(&rows, 1).is_err());
    assert_eq!(
        fs::read(path.join("loader/0000000001.json")).unwrap(),
        bytes
    );
    // Failed create-new leaves an explicit recovery entry, not an overwritten checkpoint.
    assert!(journal.ordinals().is_err());
    fs::remove_file(path.join("loader/0000000001.json.pending")).unwrap();
    fs::write(path.join("loader/0000000001.json"), b"{}").unwrap();
    assert!(journal.validate_checkpoint(&rows, 1).is_err());
    fs::remove_file(path.join("loader/0000000001.json")).unwrap();
    fs::write(path.join("loader/0000000002.json"), &bytes).unwrap();
    assert!(journal.ordinals().is_err());
    drop(journal);
    fs::remove_dir_all(path).unwrap();
}
