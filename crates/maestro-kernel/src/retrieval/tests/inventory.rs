//! Complete, pinned-generation inventory reads.

use super::support::SearchDb;
use crate::retrieval::SystemClock;
use crate::{
    evidence::{Inventory, InventoryCount},
    retrieval::{InventoryRequest, ReadControl, SearchMember, SearchRead},
};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn inventory_counts_member_metadata_and_returns_its_own_support_chunk() {
    let search = SearchDb::new("Install the tool with --force.");
    search.ready();
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version: None,
        control: &control,
    };

    let selection = search
        .database
        .inventory(&read, &InventoryRequest::DocumentsBySet { set: None })
        .unwrap();
    assert_eq!(
        selection.inventory,
        Inventory::DocumentsBySet {
            set_filter: None,
            total_documents: 1,
            sets: vec![InventoryCount {
                value: Some("guide".to_owned()),
                documents: 1,
            }],
        }
    );
    assert_eq!(
        selection
            .supports
            .iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-a"]
    );
}

#[test]
fn duplicate_documents_count_independently_without_borrowed_supports() {
    let search = SearchDb::new("Install the tool with --force.");
    search.add_duplicate_document();
    search.ready_with_members(&[
        SearchMember {
            revision_id: "rev-a".to_owned(),
            representative_revision_id: "rev-a".to_owned(),
        },
        SearchMember {
            revision_id: "rev-b".to_owned(),
            representative_revision_id: "rev-a".to_owned(),
        },
    ]);
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version: None,
        control: &control,
    };
    let selection = search
        .database
        .inventory(&read, &InventoryRequest::DocumentsBySet { set: None })
        .unwrap();
    assert_eq!(
        selection.inventory,
        Inventory::DocumentsBySet {
            set_filter: None,
            total_documents: 2,
            sets: vec![
                InventoryCount {
                    value: Some("faq".to_owned()),
                    documents: 1,
                },
                InventoryCount {
                    value: Some("guide".to_owned()),
                    documents: 1,
                },
            ],
        }
    );
    assert_eq!(selection.supports.len(), 1);
    assert_eq!(
        selection
            .supports
            .first()
            .map(|hit| hit.revision_id.as_str()),
        Some("rev-a")
    );
}

#[test]
fn inventory_applies_exact_set_and_version_filters_before_its_counts() {
    let search = SearchDb::new("Install the tool with --force.");
    search.ready();
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version: Some("9.9"),
        control: &control,
    };
    let selection = search
        .database
        .inventory(
            &read,
            &InventoryRequest::Versions {
                set: Some("guide".to_owned()),
            },
        )
        .unwrap();
    assert_eq!(
        selection.inventory,
        Inventory::Versions {
            set_filter: Some("guide".to_owned()),
            total_documents: 0,
            versions: Vec::new(),
        }
    );
    assert!(selection.supports.is_empty());
}
