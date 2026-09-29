//! Direct invariants of the current delivery-graph contract.
use super::support::graph;
use crate::{
    artifact::Digest,
    store,
    unit_graph::{ContextKind, DeliveryGraph, Error, MappingLedger, SplitMarker, UnitKind},
};
use std::error::Error as _;

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "each malformed graph is one mutation regression"
)]
fn malformed_delivery_graphs_are_refused() {
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../../../tests/fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let source = include_str!("../../../tests/fixtures/unit-graph-v1.txt");
    let mut invalid = Vec::new();

    let mut changed = graph();
    changed.groups[0].group_id = changed.groups[1].group_id.clone();
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[4].parent = Some("table".into());
    changed.groups[1]
        .children
        .retain(|child| child != "code-group");
    changed.groups[2].children.push("code-group".into());
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[0].children.push("section".into());
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[2]
        .children
        .retain(|child| child != "row-group");
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[1].parent = None;
    invalid.push(changed);
    let mut changed = graph();
    changed.descriptor.schema_version = "unsupported".into();
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[2].part_ids.swap(0, 1);
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[0].parent = Some("section".into());
    changed.groups[1].children.push("page".into());
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[2].context_relations.swap(0, 1);
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[2].context_relations[0].kind = ContextKind::LeadIn;
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[2].context_relations[0].part_id = "part-7".into();
    invalid.push(changed);
    let mut changed = graph();
    changed.groups[2].part_ids.push("part-7".into());
    invalid.push(changed);
    let mut changed = graph();
    changed.units[5].parent = Some("section".into());
    invalid.push(changed);
    let mut changed = graph();
    changed.units[5].kind = UnitKind::Block;
    invalid.push(changed);
    let mut changed = graph();
    changed.units[5].parent = None;
    invalid.push(changed);
    let mut changed = graph();
    changed.units[0].unit_id.clear();
    invalid.push(changed);
    let mut changed = graph();
    changed.units[0].part_ids.clear();
    invalid.push(changed);
    let mut changed = graph();
    changed.units[5].parent = Some("section".into());
    changed.groups[4]
        .children
        .retain(|child| child != "code-block");
    changed.groups[1].children.push("code-block".into());
    invalid.push(changed);
    for (ordinal, total) in [(0, 1), (1, 1), (2, 2)] {
        let mut changed = graph();
        changed.units[0].split = SplitMarker::Continuation { ordinal, total };
        invalid.push(changed);
    }
    let mut changed = graph();
    changed.retrieval_views[0].memberships[0]
        .primary_part_ids
        .swap(0, 1);
    invalid.push(changed);
    let mut changed = graph();
    changed.retrieval_views[0].memberships[0].primary_part_ids[1] = "part-3".into();
    invalid.push(changed);
    let mut changed = graph();
    let duplicate = changed.retrieval_views[0].memberships[0].clone();
    changed.retrieval_views[0].memberships.insert(1, duplicate);
    invalid.push(changed);
    let mut changed = graph();
    changed.retrieval_views[0].chunk_id.clear();
    invalid.push(changed);
    let mut changed = graph();
    changed.retrieval_views[0].retrieval_view_id.clear();
    invalid.push(changed);
    let mut changed = graph();
    changed.retrieval_views[0].token_count = 0;
    invalid.push(changed);
    let mut changed = graph();
    changed.retrieval_views[0].memberships[0].unit_id = "missing".into();
    invalid.push(changed);
    let mut changed = graph();
    changed.parts[0].part_id.clear();
    invalid.push(changed);
    let mut changed = graph();
    changed.parts[0].ranges.clear();
    changed.parts[0].mappings.clear();
    invalid.push(changed);
    let mut changed = graph();
    changed.parts[0].mappings.clear();
    invalid.push(changed);
    let mut changed = graph();
    changed.parts[1].part_id = "part-0".into();
    invalid.push(changed);
    let mut changed = graph();
    changed.parts[0].mappings[0].derived_range = (0, 0);
    invalid.push(changed);
    let mut changed = graph();
    changed.parts[0].ranges[0].end = changed.parts[0].ranges[0].start;
    invalid.push(changed);
    let mut changed = graph();
    changed.parts[0].ranges[0].end = 1;
    changed.parts[1].ranges[0].start = 0;
    invalid.push(changed);
    let mut changed = graph();
    changed.retrieval_views[0].memberships.clear();
    invalid.push(changed);

    let mut valid_continuation = graph();
    valid_continuation.units[0].split = SplitMarker::Continuation {
        ordinal: 0,
        total: 2,
    };
    assert!(
        valid_continuation
            .validate_recorded(&ledger, source)
            .is_ok()
    );

    for (index, changed) in invalid.iter().enumerate() {
        assert!(
            changed.validate_recorded(&ledger, source).is_err(),
            "malformed graph case {index} was accepted"
        );
    }
}

/// Digest verification rejects a bad source digest despite a matching ledger.
#[test]
fn source_digest_mismatch_is_rejected_when_mapping_digest_matches() {
    let mut graph = graph();
    let mut ledger = MappingLedger::from_bytes(
        include_bytes!("../../../tests/fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let source = include_str!("../../../tests/fixtures/unit-graph-v1.txt");
    let wrong_source_digest = Digest::of(b"different source");
    graph.descriptor.original_markdown_digest = wrong_source_digest.clone();
    ledger.original_markdown_digest = wrong_source_digest;
    graph.descriptor.mapping_digest = Digest::of(&ledger.to_bytes().unwrap());

    assert!(graph.validate(&ledger, source).is_err());
}

#[test]
fn graph_errors_format_messages_and_preserve_sources() {
    let json = Error::from(serde_json::from_slice::<DeliveryGraph>(b"{").unwrap_err());
    assert!(json.to_string().starts_with("graph JSON:"));
    assert!(json.source().is_some());

    let stored = Error::from(store::Error::Sqlite(rusqlite::Error::InvalidQuery));
    assert!(stored.source().is_some());
}
