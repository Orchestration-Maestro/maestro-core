//! Each frozen manifest and inventory guard refuses independently before journaling.
use super::{
    graph_draft::{Router, answer, authority, draft, manifest},
    support::Home,
};
use maestro_kernel::{artifact::Digest, document::Document};
use serde_json::{Value, json};
use std::{fs, path::Path};

/// Persist one manifest through the public command's strict admission boundary.
fn write_manifest(path: &Path, value: &Value) {
    fs::write(path, value.to_string()).unwrap();
}

/// A manifest refusal must not reserve any inference work.
fn refused(home: &Home, path: &Path, defect: &str) {
    let result = draft(home, path);
    assert_eq!(result.status.code(), Some(2), "{defect}: {result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("graph_manifest_invalid"),
        "{defect}: {result:?}"
    );
    assert!(!path.parent().unwrap().join("draft.lock").exists());
}

#[test]
fn graph_draft_manifest_requires_each_nonzero_limit_and_family() {
    let home = Home::bare();
    let (card, generation) = authority(&home);
    let router = Router::new(answer());
    let path = manifest(&home, &router.url, &card, generation);
    let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for (field, replacement) in [
        ("schema", json!("wrong")),
        ("family", json!(" ")),
        ("max_windows", json!(0)),
        ("max_tokens", json!(0)),
        ("max_source_bytes", json!(0)),
    ] {
        let mut value = original.clone();
        value[field] = replacement;
        write_manifest(&path, &value);
        refused(&home, &path, field);
    }
    write_manifest(&path, &original);
    assert!(draft(&home, &path).status.success());
}

#[test]
fn graph_draft_inventory_requires_each_identity_population_and_window_bound() {
    let home = Home::bare();
    let (card, generation) = authority(&home);
    let router = Router::new(answer());
    let path = manifest(&home, &router.url, &card, generation);
    let mut settings: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let inventory_path = path.parent().unwrap().join("inventory.json");
    let original: Value = serde_json::from_slice(&fs::read(&inventory_path).unwrap()).unwrap();
    for defect in [
        "schema",
        "scope",
        "collection",
        "generation",
        "empty",
        "count",
        "duplicate",
        "empty-id",
        "long-id",
        "unsafe-id",
        "span",
        "bytes",
        "version",
    ] {
        let mut value = original.clone();
        settings["max_windows"] = json!(2);
        inventory_defect(&mut value, defect);
        if defect == "count" {
            settings["max_windows"] = json!(1);
        }
        let text = value.to_string();
        fs::write(&inventory_path, &text).unwrap();
        settings["inventory_digest"] = json!(Digest::of(text.as_bytes()));
        write_manifest(&path, &settings);
        refused(&home, &path, defect);
    }
    for (id, span) in [("q-1".to_owned(), [0, 4096]), ("q".repeat(128), [1, 4097])] {
        let mut value = original.clone();
        value["windows"][0]["id"] = json!(id);
        value["windows"][0]["span"] = json!(span);
        let text = value.to_string();
        fs::write(&inventory_path, &text).unwrap();
        settings["inventory_digest"] = json!(Digest::of(text.as_bytes()));
        write_manifest(&path, &settings);
        let result = draft(&home, &path);
        assert_ne!(result.status.code(), Some(2), "exact boundary: {result:?}");
        // The model's anchor cannot validate this oversized source window, but admission succeeded.
        for entry in fs::read_dir(path.parent().unwrap()).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().to_string_lossy().starts_with("draft-") {
                fs::remove_file(entry.path()).unwrap();
            }
        }
    }
}

/// Alter just one inventory invariant while retaining a valid digest.
fn inventory_defect(value: &mut Value, defect: &str) {
    match defect {
        "schema" => value["schema"] = json!("wrong"),
        "scope" => value["scope"] = json!("pilot"),
        "collection" => value["collection"] = json!("other"),
        "generation" => value["generation"] = json!(0),
        "empty" => value["windows"] = json!([]),
        "count" | "duplicate" => {
            let mut second = value["windows"][0].clone();
            if defect == "count" {
                second["id"] = json!("q-2");
            }
            value["windows"].as_array_mut().unwrap().push(second);
        }
        "empty-id" => value["windows"][0]["id"] = json!(""),
        "long-id" => value["windows"][0]["id"] = json!("q".repeat(129)),
        "unsafe-id" => value["windows"][0]["id"] = json!("unsafe id"),
        "span" => value["windows"][0]["span"] = json!([1, 1]),
        "bytes" => value["windows"][0]["span"] = json!([1, 4098]),
        "version" => value["windows"][0]["version"] = json!(""),
        _ => panic!("unknown defect"),
    }
}

#[test]
fn graph_draft_source_metadata_and_size_are_checked_independently() {
    let home = Home::bare();
    let (card, generation) = authority(&home);
    let router = Router::new(answer());
    let path = manifest(&home, &router.url, &card, generation);
    let original_settings: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let inventory_path = path.parent().unwrap().join("inventory.json");
    let original: Value = serde_json::from_slice(&fs::read(&inventory_path).unwrap()).unwrap();
    let database = home.database();
    database
        .record_document(&Document {
            id: "doc-2".into(),
            collection_id: "synthetic".into(),
            source_id: "source".into(),
            source_ref: "another.md".into(),
        })
        .unwrap();
    let alternate = database
        .put(b"alternate source text", "text/markdown")
        .unwrap();
    for defect in [
        "generation",
        "revision",
        "document",
        "original",
        "version",
        "size",
    ] {
        let mut value = original.clone();
        let mut settings = original_settings.clone();
        match defect {
            "generation" => value["generation"] = json!(generation + 1),
            "revision" => value["windows"][0]["revision_id"] = json!("absent"),
            "document" => value["windows"][0]["document_id"] = json!("doc-2"),
            "original" => value["windows"][0]["original"] = json!(alternate),
            "version" => value["windows"][0]["version"] = json!("2.0"),
            _ => settings["max_source_bytes"] = json!(1),
        }
        let text = value.to_string();
        fs::write(&inventory_path, &text).unwrap();
        settings["inventory_digest"] = json!(Digest::of(text.as_bytes()));
        write_manifest(&path, &settings);
        let result = draft(&home, &path);
        assert_eq!(result.status.code(), Some(2), "{defect}: {result:?}");
        assert!(String::from_utf8_lossy(&result.stderr).contains("graph_labels_invalid"));
    }
    let size = original["windows"][0]["span"][1].as_u64().unwrap();
    let mut settings = original_settings;
    settings["max_source_bytes"] = json!(size);
    let text = original.to_string();
    fs::write(&inventory_path, &text).unwrap();
    settings["inventory_digest"] = json!(Digest::of(text.as_bytes()));
    write_manifest(&path, &settings);
    assert!(draft(&home, &path).status.success());
}

#[test]
fn graph_draft_admits_exact_metadata_file_limits() {
    let home = Home::bare();
    let (card, generation) = authority(&home);
    let router = Router::new(answer());
    let path = manifest(&home, &router.url, &card, generation);
    let input = path.parent().unwrap().join("inventory.json");
    let mut inventory = fs::read_to_string(&input).unwrap();
    inventory.push_str(&" ".repeat(16 * 1024 * 1024 - inventory.len()));
    fs::write(input, &inventory).unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["inventory_digest"] = json!(Digest::of(inventory.as_bytes()));
    let mut settings = value.to_string();
    settings.push_str(&" ".repeat(1024 * 1024 - settings.len()));
    fs::write(&path, settings).unwrap();
    let output = draft(&home, &path);
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn graph_draft_checks_recorded_and_actual_source_sizes_independently() {
    for overreported in [false, true] {
        let home = Home::bare();
        let (card, generation) = authority(&home);
        let router = Router::new(answer());
        let path = manifest(&home, &router.url, &card, generation);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let inventory: Value = serde_json::from_slice(
            &fs::read(path.parent().unwrap().join("inventory.json")).unwrap(),
        )
        .unwrap();
        let size = inventory["windows"][0]["span"][1].as_u64().unwrap();
        let recorded = if overreported { size + 1 } else { 1 };
        value["max_source_bytes"] = json!(if overreported { size } else { size - 1 });
        write_manifest(&path, &value);
        let connection = rusqlite::Connection::open(home.data().join("kernel.sqlite3")).unwrap();
        connection
            .execute(
                "UPDATE artifacts SET bytes = ?1 WHERE digest = ?2",
                rusqlite::params![
                    i64::try_from(recorded).unwrap(),
                    inventory["windows"][0]["original"].as_str().unwrap()
                ],
            )
            .unwrap();
        drop(connection);
        let result = draft(&home, &path);
        assert_eq!(
            result.status.code(),
            Some(2),
            "overreported={overreported}: {result:?}"
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("graph_labels_invalid"));
    }
}
