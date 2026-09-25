//! Saved snapshots: publication, recovery, verified loading and the roots
//! neither assets nor snapshots can escape.
use super::fixture::Fixture;
use maestro_canonicalization::{CanonicalizeInput, canonicalize, load_document, save_document};
use serde_json::Value;
use std::{fs, path::Path, thread};

#[test]
fn cli_refuses_to_overwrite_a_corrupted_reference_copy() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("input.md"), "Original\n").unwrap();
    let first = fixture.run(&[]);
    assert!(first.status.success());
    let (path, _) = fixture.result(&first);
    let copy = path.parent().unwrap().join("original.md");
    fs::write(&copy, "tampered").unwrap();
    let next = fixture.run(&[]);
    assert_eq!(next.status.code(), Some(1));
    assert_eq!(fs::read_to_string(copy).unwrap(), "tampered");
}

#[test]
fn revisions_and_partial_publications_remain_recoverable() {
    let fixture = Fixture::new();
    let old = "# Café 🦀\r\n\r\nDo not change repoName in 9.0.22.\r\n";
    fs::write(fixture.0.join("input.md"), old).unwrap();
    let (old_path, old_doc) = fixture.result(&fixture.run(&[]));
    let old_json = fs::read(&old_path).unwrap();
    assert_eq!(
        load_document(&old_path).unwrap().1.as_bytes(),
        old.as_bytes()
    );
    fs::remove_file(&old_path).unwrap();
    fs::write(old_path.with_extension("pending-interrupted"), "partial").unwrap();
    assert!(load_document(&old_path).is_err());
    let resumed = fixture.run(&[]);
    assert!(resumed.status.success());
    assert_eq!(fixture.result(&resumed).0, old_path);
    assert_eq!(fs::read(&old_path).unwrap(), old_json);
    let new = old.replace("9.0.22", "9.0.23");
    fs::write(fixture.0.join("input.md"), &new).unwrap();
    let (new_path, new_doc) = fixture.result(&fixture.run(&[]));
    assert_eq!(new_doc["document_id"], old_doc["document_id"]);
    assert_ne!(new_doc["revision_id"], old_doc["revision_id"]);
    assert_ne!(new_path, old_path);
    assert_eq!(load_document(&old_path).unwrap().1, old);
    assert_eq!(load_document(&new_path).unwrap().1, new);
    assert_eq!(fs::read(&old_path).unwrap(), old_json);
    assert_eq!(
        fs::read(fixture.0.join("input.md")).unwrap(),
        new.as_bytes()
    );
}

#[test]
fn verified_loader_refuses_tampered_and_incomplete_snapshots() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("input.md"), "Original\n").unwrap();
    let (path, doc) = fixture.result(&fixture.run(&[]));
    let json = fs::read(&path).unwrap();
    assert!(load_document(&path).is_ok());
    let copy = path.parent().unwrap().join("original.md");
    check_a_bad_original_copy_is_refused_and_kept(&fixture, &path, &copy, &json);
    check_a_missing_original_copy_is_refused_then_restored(&fixture, &path, &copy);
    check_tampered_identities_are_refused(&path, &doc);
    check_bad_snapshot_json_is_refused_and_kept(&fixture, &path);
    fs::write(&path, &json).unwrap();
    let outside = fixture.0.join("canonical.json");
    fs::write(&outside, &json).unwrap();
    assert!(load_document(&outside).is_err());
    assert!(load_document(&path).is_ok());
}

/// A tampered, non-UTF-8 or empty original copy fails to load and makes the
/// tool exit 1; neither file is rewritten.
fn check_a_bad_original_copy_is_refused_and_kept(
    fixture: &Fixture,
    path: &Path,
    copy: &Path,
    json: &[u8],
) {
    for bad in [b"tampered".as_slice(), &[0xff], b""] {
        fs::write(copy, bad).unwrap();
        assert!(load_document(path).is_err());
        assert_eq!(fixture.run(&[]).status.code(), Some(1));
        assert_eq!(fs::read(copy).unwrap(), bad);
        assert_eq!(fs::read(path).unwrap(), json);
    }
}

/// A missing original copy fails to load; the tool then writes it again.
fn check_a_missing_original_copy_is_refused_then_restored(
    fixture: &Fixture,
    path: &Path,
    copy: &Path,
) {
    fs::write(copy, "Original\n").unwrap();
    fs::remove_file(copy).unwrap();
    assert!(load_document(path).is_err());
    assert!(fixture.run(&[]).status.success());
}

/// A snapshot whose identity fields or reference path changed fails to load.
fn check_tampered_identities_are_refused(path: &Path, doc: &Value) {
    for field in [
        "schema_version",
        "revision_id",
        "parser_version",
        "access_policy",
    ] {
        let mut changed = doc.clone();
        changed[field] = Value::String("tampered".into());
        fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(load_document(path).is_err(), "{field}");
    }
    let mut changed = doc.clone();
    changed["original_markdown_reference"]["path"] = Value::String("../input.md".into());
    fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(load_document(path).is_err());
}

/// Malformed or duplicate-key JSON fails to load, makes the tool exit 1 and
/// is kept as it is.
fn check_bad_snapshot_json_is_refused_and_kept(fixture: &Fixture, path: &Path) {
    for bad in [
        b"{".as_slice(),
        b"{}",
        b"{\"document_id\":\"one\",\"document_id\":\"two\"}",
    ] {
        fs::write(path, bad).unwrap();
        assert!(load_document(path).is_err());
        assert_eq!(fixture.run(&[]).status.code(), Some(1));
        assert_eq!(fs::read(path).unwrap(), bad);
    }
}

#[test]
#[cfg(unix)]
fn symlinks_and_traversal_cannot_escape_asset_or_snapshot_roots() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.0.join("secret.svg"), "never read this asset").unwrap();
    symlink(outside.0.join("secret.svg"), fixture.0.join("escape.svg")).unwrap();
    let md = "![x](escape.svg) ![y](%2e%2e/outside.svg)\n";
    fs::write(fixture.0.join("input.md"), md).unwrap();
    let result = fixture.run(&[]);
    assert!(result.status.success());
    let (path, value) = fixture.result(&result);
    assert_eq!(value["asset_inventory"]["escape.svg"], "outside_root");
    assert_eq!(
        value["asset_inventory"]["%2e%2e/outside.svg"],
        "outside_root"
    );
    let (doc, original) = load_document(&path).unwrap();
    assert!(save_document(&doc, &original, &fixture.0.join("traverse/../out")).is_err());
    let link = fixture.0.join("linked-snapshot");
    symlink(path.parent().unwrap(), &link).unwrap();
    assert!(load_document(&link.join("canonical.json")).is_err());
    let copy = path.parent().unwrap().join("original.md");
    fs::remove_file(&copy).unwrap();
    symlink(outside.0.join("secret.svg"), &copy).unwrap();
    assert!(load_document(&path).is_err());
    assert!(save_document(&doc, &original, &fixture.0.join("out")).is_err());
    fs::remove_file(&copy).unwrap();
    fs::write(&copy, md).unwrap();
    fs::remove_file(&path).unwrap();
    symlink(&copy, &path).unwrap();
    assert!(load_document(&path).is_err());
    assert!(save_document(&doc, &original, &fixture.0.join("out")).is_err());
    assert_eq!(
        fs::read_to_string(outside.0.join("secret.svg")).unwrap(),
        "never read this asset"
    );
}

#[test]
#[cfg(unix)]
fn a_root_reached_through_a_link_is_accepted_but_no_link_below_it() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let md = "Linked root\n";
    let doc = canonicalize(CanonicalizeInput::new(md, "linked.md")).unwrap();
    symlink(&outside.0, fixture.0.join("linked-output")).unwrap();
    let path = save_document(&doc, md, &fixture.0.join("linked-output/new")).unwrap();
    assert!(path.starts_with(fixture.0.join("linked-output/new")));
    assert!(outside.0.join("new").is_dir());
    assert_eq!(load_document(&path).unwrap().1, md);
    // A link planted under the root, in place of an identity directory, is refused.
    let identity = path.ancestors().nth(3).unwrap();
    let moved = outside.0.join("moved");
    fs::rename(identity, &moved).unwrap();
    symlink(&moved, identity).unwrap();
    assert!(load_document(&path).is_err());
    assert!(save_document(&doc, md, &fixture.0.join("linked-output/new")).is_err());
    let snapshot = moved.join(path.parent().unwrap().strip_prefix(identity).unwrap());
    assert_eq!(fs::read_dir(snapshot).unwrap().count(), 2);
}

#[test]
fn concurrent_identical_writers_publish_one_valid_snapshot() {
    let fixture = Fixture::new();
    let md = "Do not change 9.0.22.\n";
    let doc = canonicalize(CanonicalizeInput::new(md, "shared.md")).unwrap();
    let paths = thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| save_document(&doc, md, &fixture.0)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(paths.iter().all(|path| path == &paths[0]));
    assert_eq!(load_document(&paths[0]).unwrap().1, md);
    assert_eq!(fs::read_dir(paths[0].parent().unwrap()).unwrap().count(), 2);
}
