use super::{
    super::{EvidenceCounter, assemble_evidence},
    support::{Fixture, evidence_input, fixture},
};
use crate::{
    prepare::tests::scratch::{revision_of, set_revision_metadata},
    query::QueryKind,
    search::EvidenceInput,
};
use maestro_kernel::evidence::{Bundle, Conflict, Inventory, InventoryCount, RouteStatus};
use std::{collections::BTreeSet, sync::Arc};

const QUERY: &str = "How do I install the Agent and what port does it use?";

fn versioned_fixture() -> Fixture {
    let shared = (0..160)
        .map(|index| format!("sharedword{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let old = markdown(&shared, "7005");
    let current = markdown(&shared, "7006");
    let fixture = fixture(&[("version-1.md", &old), ("version-2.md", &current)]);
    let old_revision = revision_of(&fixture.database, &fixture.scopes, "version-1.md");
    let current_revision = revision_of(&fixture.database, &fixture.scopes, "version-2.md");
    set_revision_metadata(&fixture.scratch, &old_revision, "version", "1.0");
    set_revision_metadata(&fixture.scratch, &current_revision, "version", "2.0");
    fixture
}

fn markdown(shared: &str, port: &str) -> String {
    format!(
        "# Guide

## Install

{shared}

## Port

| Entity | Attribute | Value |
| --- | --- | --- |
| Agent | Port | {port} |
"
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identical_latest_sections_keep_older_provenance_and_conflicts() {
    let fixture = versioned_fixture();
    let input = evidence_input(&fixture, QUERY);
    let database = Arc::new(fixture.database);

    let bundle = assemble_evidence(database, input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();

    let latest = bundle
        .passages
        .iter()
        .find(|passage| passage.version.as_deref() == Some("2.0"))
        .expect("the latest passage is returned");
    assert_eq!(latest.alternates.len(), 1);
    assert_eq!(latest.alternates[0].version.as_deref(), Some("1.0"));
    assert!(bundle.known_gaps.is_empty());

    let table_passages = bundle
        .passages
        .iter()
        .filter(|passage| passage.text.contains("| Agent | Port |"))
        .map(|passage| passage.n)
        .collect::<Vec<_>>();
    assert_eq!(table_passages.len(), 2);
    assert_eq!(
        bundle.conflicts,
        vec![Conflict {
            entity: "Agent".to_owned(),
            attribute: "Port".to_owned(),
            passages: table_passages,
        }]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_single_passage_budget_omits_conflicts_as_one_unit_with_exact_gaps() {
    let fixture = versioned_fixture();
    let input = table_only_input(&fixture);

    let bundle = assemble_evidence(
        Arc::new(fixture.database),
        input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap();

    assert!(bundle.passages.is_empty());
    assert_eq!(
        bundle.known_gaps,
        [
            "No accessible evidence was returned for this query in the pinned generation.",
            "Some candidate evidence was omitted by the evidence or passage budget.",
            concat!(
                "Conflicting values could not be retained together within the evidence ",
                "or passage budget."
            ),
        ]
    );
}

fn table_only_input(fixture: &Fixture) -> EvidenceInput {
    let mut input = evidence_input(fixture, QUERY);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let mut table_ids = BTreeSet::new();
    for chunk in chunks {
        let revision = fixture
            .database
            .revision(&fixture.scopes, &chunk.revision_id)
            .unwrap()
            .unwrap();
        let markdown =
            String::from_utf8(fixture.database.get(&revision.original_digest).unwrap()).unwrap();
        let Some(text) = markdown.get(chunk.span.start..chunk.span.end) else {
            continue;
        };
        if text.contains("| Agent | Port |") {
            table_ids.insert(chunk.id);
        }
    }
    input
        .ranked
        .retain(|ranked| table_ids.contains(&ranked.candidate.fused.chunk_id));
    input.budget.k = 1;
    input
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_filter_comparison_and_version_inventory_keep_their_distinct_versions() {
    let fixture = versioned_fixture();
    let mut filtered_input = evidence_input(&fixture, QUERY);
    filtered_input.version = Some("1.0".to_owned());
    let mut comparison_input = evidence_input(&fixture, "Compare the Agent port in both versions.");
    comparison_input.understood.kind = QueryKind::Comparison;
    let mut inventory_input = evidence_input(&fixture, QUERY);
    inventory_input
        .routes
        .insert("structured".to_owned(), RouteStatus::Ok);
    inventory_input.inventory = Some(Inventory::Versions {
        set_filter: None,
        total_documents: 2,
        versions: vec![
            InventoryCount {
                value: Some("1.0".to_owned()),
                documents: 1,
            },
            InventoryCount {
                value: Some("2.0".to_owned()),
                documents: 1,
            },
        ],
    });
    let database = Arc::new(fixture.database);

    let filtered = assemble_evidence(database.clone(), filtered_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();
    assert!(
        filtered
            .passages
            .iter()
            .all(|passage| passage.version.as_deref() == Some("1.0"))
    );
    assert!(
        filtered
            .passages
            .iter()
            .all(|passage| passage.alternates.is_empty())
    );
    assert_eq!(
        filtered.known_gaps,
        ["One or more candidate passages could not be resolved in the current scopes."]
    );

    let comparison = assemble_evidence(
        database.clone(),
        comparison_input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap();
    assert_distinct_versions(&comparison);

    let inventory = assemble_evidence(database, inventory_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();
    assert_distinct_versions(&inventory);
    assert_eq!(
        inventory.inventory,
        Some(Inventory::Versions {
            set_filter: None,
            total_documents: 2,
            versions: vec![
                InventoryCount {
                    value: Some("1.0".to_owned()),
                    documents: 1,
                },
                InventoryCount {
                    value: Some("2.0".to_owned()),
                    documents: 1,
                },
            ],
        })
    );
}

fn assert_distinct_versions(bundle: &Bundle) {
    let versions = bundle
        .passages
        .iter()
        .filter_map(|passage| passage.version.as_deref())
        .collect::<BTreeSet<_>>();
    assert_eq!(versions, ["1.0", "2.0"].into_iter().collect());
    assert!(
        bundle
            .passages
            .iter()
            .all(|passage| passage.alternates.is_empty())
    );
}
