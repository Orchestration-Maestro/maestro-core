//! `maestro status` as its users run it: which services are ready, and the
//! collections the local principal reads with their published generation;
//! it answers whatever is down, and creates nothing.

use super::{machine::checked, support::Home};
use maestro_kernel::generation::NewGeneration;
use rusqlite::Connection;
use serde_json::{Value, json};

/// The names of the services `document` reports, and whether each is
/// ready.
fn services(document: &Value) -> Vec<(&str, bool)> {
    document["services"]
        .as_array()
        .unwrap()
        .iter()
        .map(|service| {
            (
                service["name"].as_str().unwrap(),
                service["ready"].as_bool().unwrap(),
            )
        })
        .collect()
}

#[test]
fn status_summarizes_the_services_and_the_collections_the_local_principal_reads() {
    let home = Home::new();
    home.add_synthetic();
    let ended = checked(&home, &["status", "--json"]);
    assert_eq!(
        (ended.code, ended.stderr.as_str()),
        (Some(0), ""),
        "{ended:?}"
    );
    let document = ended.json();
    assert_eq!(document["schema"], "maestro-cli/status/1");
    let services = services(&document);
    let names: Vec<&str> = services.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["kernel", "qdrant", "router"]);
    assert_eq!(services[0], ("kernel", true));
    assert_eq!(
        services[2],
        ("router", false),
        "nothing answers where it looks"
    );
    assert_eq!(
        document["collections"],
        json!([{
            "collection": "synthetic",
            "title": "Synthetic operations handbook in French and English, \
                      written for public tests",
            "documents": 0,
            "published": null,
        }])
    );
    let text = checked(&home, &["status"]);
    assert_eq!(text.code, Some(0), "{text:?}");
    assert!(
        text.stdout
            .contains("collection synthetic: 0 documents, no generation published")
            && text.stdout.contains("maestro doctor"),
        "{}",
        text.stdout
    );
}

#[test]
fn status_on_a_fresh_machine_answers_and_creates_nothing() {
    let home = Home::bare();
    let ended = checked(&home, &["status", "--json"]);
    assert_eq!(ended.code, Some(0), "{ended:?}");
    let document = ended.json();
    assert_eq!(services(&document)[0], ("kernel", false));
    assert_eq!(document["collections"], json!([]));
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn status_names_the_published_generation_of_a_collection() {
    let home = Home::new();
    home.add_synthetic();
    // A chunk set as preparation records it: begun building, then complete
    // with its manifest.
    Connection::open(home.data().join("kernel.sqlite3"))
        .unwrap()
        .execute_batch(
            "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
             VALUES ('synthetic-set', 'synthetic', 'structural-500-700/1', 'native', 'building');
             UPDATE chunk_sets SET state = 'complete',
               manifest_digest = '4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945'
             WHERE id = 'synthetic-set';",
        )
        .unwrap();
    let database = home.database();
    let new = NewGeneration {
        collection_id: "synthetic".to_owned(),
        chunk_set_id: "synthetic-set".to_owned(),
        embedding_profile: "embed:test".to_owned(),
        sparse_profile: "bm25-en-fr/1".to_owned(),
    };
    let published = database.create_generation(&new).unwrap();
    database.verify_generation(published.id, 3).unwrap();
    database.publish_generation(published.id).unwrap();
    database.create_generation(&new).unwrap();
    let document = checked(&home, &["status", "--json"]).json();
    assert_eq!(
        document["collections"][0]["published"],
        json!({"generation": published.id, "points": 3}),
        "the published generation, not the one still building"
    );
    let text = checked(&home, &["status"]).stdout;
    assert!(
        text.contains(&format!(
            "collection synthetic: 0 documents, generation {} published",
            published.id
        )),
        "{text}"
    );
}
