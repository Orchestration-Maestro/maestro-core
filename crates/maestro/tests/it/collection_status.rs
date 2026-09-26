//! `knowledge status`: a collection's documents, its revisions by status and
//! by quality disposition, and its generations, as the local principal reads
//! them.

use super::support::{Home, local};
use maestro_kernel::{
    document::{Disposition, Outcome},
    generation::NewGeneration,
};
use rusqlite::Connection;
use serde_json::{Map, Value, json};

/// The title the synthetic collection's declaration gives it.
const TITLE: &str = "Synthetic operations handbook in French and English, written for public tests";

#[test]
fn status_reports_documents_revisions_dispositions_and_generations() {
    let home = Home::new();
    home.add_synthetic();
    let imported = home.run(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(imported.code, Some(0), "{imported:?}");
    let database = home.database();
    let scopes = local(&database);
    let revisions = database.eligible_revisions(&scopes, "synthetic").unwrap();
    let accepted = Disposition {
        revision_id: revisions[0].id.clone(),
        outcome: Outcome::Accepted,
        reasons: vec!["checked".to_owned()],
        rule_ids: vec!["quality.test".to_owned()],
        decided_by: "test".to_owned(),
    };
    database.record_disposition(&accepted).unwrap();
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
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "synthetic".to_owned(),
            chunk_set_id: "synthetic-set".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    database.verify_generation(generation.id, 3).unwrap();
    database.publish_generation(generation.id).unwrap();
    let published = database
        .generation(&scopes, generation.id)
        .unwrap()
        .unwrap();
    let counts = database.collection_counts(&scopes, "synthetic").unwrap();
    let statuses: Map<String, Value> = counts
        .statuses
        .iter()
        .map(|(status, count)| (status.to_string(), json!(count)))
        .collect();
    assert_eq!(statuses.values().filter_map(Value::as_u64).sum::<u64>(), 28);
    let status = home.run(&["knowledge", "status", "--collection", "synthetic", "--json"]);
    assert_eq!(
        (status.code, status.stderr.as_str()),
        (Some(0), ""),
        "{status:?}"
    );
    assert_eq!(
        status.json(),
        json!({
            "schema": "maestro-cli/knowledge-status/1",
            "collection": "synthetic",
            "title": TITLE,
            "documents": 28,
            "revisions": statuses,
            "dispositions": {
                "accepted": 1,
                "accepted_with_warnings": 0,
                "needs_reextraction": 0,
                "quarantined": 0,
                "excluded": 0,
                "undecided": 27,
            },
            "generations": [{
                "id": generation.id,
                "state": "published",
                "chunk_set": "synthetic-set",
                "embedding_profile": "embed:test",
                "sparse_profile": "bm25-en-fr/1",
                "point_count": 3,
                "published_at": published.published_at,
            }],
        })
    );
    let text = home.run(&["knowledge", "status", "--collection", "synthetic"]);
    assert_eq!(text.code, Some(0), "{text:?}");
    for line in [
        "documents 28".to_owned(),
        "dispositions accepted 1, accepted_with_warnings 0, needs_reextraction 0, \
         quarantined 0, excluded 0, undecided 27"
            .to_owned(),
        format!(
            "generation {} published: chunk set synthetic-set, 3 points",
            generation.id
        ),
    ] {
        assert!(
            text.stdout.lines().any(|each| each == line),
            "{line}: {text:?}"
        );
    }
}
