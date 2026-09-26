//! An import is idempotent: a second one finds every revision unchanged and
//! writes nothing but its completion, a change of metadata alone gives the
//! same document a new revision, and an import that stopped part way
//! continues where it stopped.

use super::support::{
    Contents, Scratch, data_of, events, everything, import, line, markdown, page,
};
use maestro_kernel::{
    artifact::Digest,
    document::{Revision, RevisionStatus},
    store::Database,
};
use serde_json::{Map, Value, json};
use std::slice;

/// The type of the event an import journals when it completes.
const COMPLETED: &str = "maestro.knowledge.import.completed.v1";

/// Every revision of `garden`, in the order they were recorded.
fn revisions(database: &Database) -> Vec<Revision> {
    database
        .eligible_revisions(&everything(database), "garden")
        .unwrap()
}

#[test]
fn a_second_import_finds_every_revision_unchanged_and_writes_nothing() {
    let scratch = Scratch::new();
    let [compost, mulch, older, newer] =
        ["compost", "mulch", "pruning, 2025", "pruning, 2026"].map(markdown);
    for (path, bytes) in [
        ("compost.md", &compost),
        ("mulch.md", &mulch),
        ("pruning-2025.md", &older),
        ("pruning.md", &newer),
    ] {
        scratch.put(path, bytes);
    }
    scratch.manifest(&[
        line("compost.md", &compost, &page("compost")),
        line("pruning-2025.md", &older, &page("pruning")),
        line(
            "mulch.md",
            &markdown("mulch, before an edit"),
            &page("mulch"),
        ),
        line("pruning.md", &newer, &page("pruning")),
        line("mulch.md", &mulch, &page("mulched-beds")),
    ]);
    let database = scratch.database();
    let first = import(&scratch, &database, "garden");
    assert_eq!(
        [first.imported, first.unchanged, first.held, first.refused],
        [2, 0, 2, 1]
    );
    let before = Contents::of(&scratch);
    let journaled = events(&database, "garden").len();
    let second = import(&scratch, &database, "garden");
    assert_eq!(
        [
            second.imported,
            second.unchanged,
            second.held,
            second.refused
        ],
        [0, 4, 0, 1]
    );
    assert_eq!(second.refusals, first.refusals);
    assert_eq!(
        Contents::of(&scratch),
        before,
        "the second import wrote nothing"
    );
    let after = events(&database, "garden");
    assert_eq!(after.len(), journaled + 1, "but its completion");
    assert_eq!(
        data_of(&after, COMPLETED).last(),
        Some(&json!({
            "collection": "garden", "imported": 0, "unchanged": 4, "held": 0, "refused": 1,
        }))
    );
}

#[test]
fn a_change_of_metadata_alone_gives_the_same_document_a_new_revision() {
    let scratch = Scratch::new();
    let pruning = markdown("pruning");
    scratch.put("pruning.md", &pruning);
    let given = line("pruning.md", &pruning, &page("pruning"));
    scratch.manifest(slice::from_ref(&given));
    let database = scratch.database();
    import(&scratch, &database, "garden");
    let mut released = given;
    released["version"] = json!("2.0");
    released["set"] = json!("orchard");
    scratch.manifest(&[released]);
    let report = import(&scratch, &database, "garden");
    assert_eq!(
        [
            report.imported,
            report.unchanged,
            report.held,
            report.refused
        ],
        [1, 0, 0, 0]
    );
    let recorded = revisions(&database);
    let [first, second] = recorded.as_slice() else {
        panic!("{recorded:#?}");
    };
    assert_eq!(first.document_id, second.document_id, "one document");
    assert_ne!(first.id, second.id);
    assert_eq!(first.original_digest, Digest::of(&pruning));
    assert_eq!(second.original_digest, first.original_digest);
    let pins = database
        .artifact(&first.original_digest)
        .unwrap()
        .unwrap()
        .pins;
    assert_eq!(pins, 2, "each revision pins the one original");
    let metadata = |version: &str, set: Option<&str>| {
        let mut metadata = Map::from_iter([
            ("title".to_owned(), Value::from("Notes on pruning.md")),
            ("source_kind".to_owned(), Value::from("guide")),
            ("version".to_owned(), Value::from(version)),
        ]);
        if let Some(set) = set {
            metadata.insert("set".to_owned(), Value::from(set));
        }
        metadata
    };
    assert_eq!(first.metadata, metadata("1.0", None));
    assert_eq!(second.metadata, metadata("2.0", Some("orchard")));
}

#[test]
fn every_field_of_a_line_describes_its_revision() {
    let scratch = Scratch::new();
    let pruning = markdown("pruning");
    scratch.put("pruning.md", &pruning);
    let mut described = line("pruning.md", &pruning, &page("pruning"));
    for (key, value) in [
        ("set", json!("orchard")),
        ("lang", json!("en")),
        ("captured_at", json!("2026-09-13T16:34:20Z")),
        ("product", json!("garden-planner")),
        ("component", json!("beds")),
        ("platform", json!("any")),
        ("extractor", json!({ "name": "hand", "version": "1" })),
        ("access", json!({ "visibility": "public" })),
    ] {
        described[key] = value;
    }
    scratch.manifest(&[described.clone()]);
    let database = scratch.database();
    import(&scratch, &database, "garden");
    let recorded = revisions(&database);
    let [revision] = recorded.as_slice() else {
        panic!("{recorded:#?}");
    };
    let Value::Object(mut expected) = described else {
        panic!("a line is an object");
    };
    for identity in ["schema", "path", "sha256", "bytes", "source_ref"] {
        expected.remove(identity);
    }
    assert_eq!(revision.metadata, expected);
    assert_eq!(
        revision.captured_at.as_deref(),
        Some("2026-09-13T16:34:20Z")
    );
    assert_eq!(
        revision.status,
        RevisionStatus::Valid,
        "its access is known"
    );
    let changes = [
        ("title", json!("Pruning")),
        ("source_kind", json!("runbook")),
        ("lang", json!("fr")),
        ("captured_at", json!("2026-09-14T00:00:00Z")),
        ("product", json!("orchard-planner")),
        ("component", json!("trees")),
        ("platform", json!("linux")),
        ("extractor", json!({ "name": "hand", "version": "2" })),
        ("access", json!({ "visibility": "private" })),
    ];
    for (key, value) in changes {
        let mut changed = expected.clone();
        changed.insert(key.to_owned(), value);
        let mut text = Value::Object(changed);
        text["schema"] = json!("maestro-corpus/1");
        text["path"] = json!("pruning.md");
        text["sha256"] = json!(Digest::of(&pruning).as_str());
        text["bytes"] = json!(pruning.len());
        text["source_ref"] = json!(page("pruning"));
        scratch.manifest(&[text]);
        let report = import(&scratch, &database, "garden");
        assert_eq!(
            report.imported, 1,
            "a change of {key} alone gives a new revision"
        );
    }
}

#[test]
fn canonicalization_gives_each_revision_its_status() {
    let scratch = Scratch::new();
    let [described, unknown] = ["a described page", "a page of unknown access"].map(markdown);
    let conflicting = b"---\ntitle: Another title\n---\n# Pruning\n".to_vec();
    scratch.put("described.md", &described);
    scratch.put("unknown.md", &unknown);
    scratch.put("conflicting.md", &conflicting);
    let mut whole = line("described.md", &described, &page("described"));
    whole["lang"] = json!("en");
    whole["extractor"] = json!({ "name": "hand" });
    whole["access"] = json!({ "visibility": "public" });
    scratch.manifest(&[
        whole,
        line("unknown.md", &unknown, &page("unknown")),
        line("conflicting.md", &conflicting, &page("conflicting")),
    ]);
    let database = scratch.database();
    let report = import(&scratch, &database, "garden");
    assert_eq!(
        report.imported, 3,
        "a failed revision is recorded, for inspection"
    );
    let statuses: Vec<RevisionStatus> = revisions(&database)
        .into_iter()
        .map(|revision| revision.status)
        .collect();
    assert_eq!(
        statuses,
        [RevisionStatus::Valid, RevisionStatus::ValidWithWarnings],
        "a failed revision is never eligible"
    );
    let canonical = database
        .get(&revisions(&database)[0].canonical_digest)
        .unwrap();
    let document: Value = serde_json::from_slice(&canonical).unwrap();
    assert_eq!(document["validation_status"], "valid");
    assert!(
        canonical.ends_with(b"}\n"),
        "saved as maestro-canonicalization saves it"
    );
}

#[test]
fn an_import_that_stopped_part_way_continues_where_it_stopped() {
    let scratch = Scratch::new();
    let documents = ["compost", "mulch", "pruning", "seeds"].map(|topic| {
        let bytes = markdown(topic);
        let path = format!("{topic}.md");
        scratch.put(&path, &bytes);
        line(&path, &bytes, &page(topic))
    });
    scratch.manifest(&documents[..2]);
    let database = scratch.database();
    import(&scratch, &database, "garden");
    scratch.manifest(&documents);
    let report = import(&scratch, &database, "garden");
    assert_eq!(
        [
            report.imported,
            report.unchanged,
            report.held,
            report.refused
        ],
        [2, 2, 0, 0]
    );
    assert_eq!(revisions(&database).len(), 4);
}
