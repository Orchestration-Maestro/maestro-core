//! The shared B06/B07 wire fixture is a canonical, validated CAS payload.

#![cfg(test)]

use maestro_kernel::{
    artifact::Digest,
    unit_graph::{
        DeliveryGraph, Exclusion, GroupKind, MappingContribution, MappingEntry, MappingLedger,
        Part, SourceRange, SplitMarker,
    },
};

/// Repository text hygiene adds exactly one LF; CAS serialization does not.
fn graph_bytes() -> &'static [u8] {
    include_bytes!("../fixtures/unit-graph-v1.json")
        .strip_suffix(b"\n")
        .unwrap()
}

#[test]
fn unit_graph_wire_golden_round_trip() {
    let mapping = include_bytes!("../fixtures/unit-mapping-v1.json")
        .strip_suffix(b"\n")
        .unwrap();
    let graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    let ledger = MappingLedger::from_bytes(mapping).unwrap();
    graph
        .validate(&ledger, include_str!("../fixtures/unit-graph-v1.txt"))
        .unwrap();
    assert_eq!(graph.to_bytes().unwrap(), graph_bytes());
    assert!(
        graph
            .to_bytes()
            .unwrap()
            .windows(b"coverage".len())
            .all(|window| window != b"coverage")
    );
    assert_eq!(ledger.to_bytes().unwrap(), mapping);
    assert_eq!(
        Digest::of(graph_bytes()).as_str(),
        "8717210bb0b68d26265e7c2478d3a10c32932bc811bb068ba14f093672a2d01d"
    );
    assert_eq!(
        Digest::of(mapping).as_str(),
        "de4b7353da814a92f84c8ffcf8cd03a055d7dc8d770e62bc09b282a2431bb4dd"
    );
}

#[test]
fn unit_graph_wire_refuses_noncanonical_and_duplicate_fields() {
    let original = String::from_utf8(graph_bytes().to_vec()).unwrap();
    for changed in [
        format!("{original}\n"),
        original.replacen("\"units\":", "\"unknown\":0,\"units\":", 1),
        original.replacen("\"units\":", "\"units\":[],\"units\":", 1),
    ] {
        assert!(DeliveryGraph::from_bytes(changed.as_bytes()).is_err());
    }
}

#[test]
fn unit_graph_wire_rejects_unaccounted_or_unlinked_content() {
    let mapping = include_bytes!("../fixtures/unit-mapping-v1.json")
        .strip_suffix(b"\n")
        .unwrap();
    let ledger = MappingLedger::from_bytes(mapping).unwrap();
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let original: serde_json::Value = serde_json::from_slice(graph_bytes()).unwrap();
    for (pointer, value) in [
        ("/units/0/part_ids/0", serde_json::json!("missing")),
        ("/parts/0/ranges/0/end", serde_json::json!(999)),
        ("/parts/0/ranges/0/end", serde_json::json!(6)),
        ("/groups/0/parent", serde_json::json!("table")),
        ("/groups/0/children/0", serde_json::json!("absent")),
        ("/parts/0/mappings/0/unit_id", serde_json::json!("wrong")),
        (
            "/retrieval_views/0/memberships/0/primary_part_ids/0",
            serde_json::json!("part-0"),
        ),
        (
            "/groups/2/context_relations/0/part_id",
            serde_json::json!("part-0"),
        ),
        (
            "/groups/0/family/collection_id",
            serde_json::json!("foreign"),
        ),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let graph: DeliveryGraph = serde_json::from_value(changed).unwrap();
        assert!(graph.validate(&ledger, source).is_err(), "{pointer}");
    }
}

#[test]
fn unit_graph_wire_refuses_duplicate_primary_owners() {
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    let part_id = graph.units[0].part_ids[0].clone();
    graph.units[1].part_ids.push(part_id);
    assert_eq!(
        graph.validate(&ledger, source).unwrap_err().to_string(),
        "invalid graph: part has multiple primary owners"
    );
}

#[test]
fn unit_graph_wire_refuses_unowned_primary_parts() {
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    graph.units[3].part_ids.remove(0);
    assert_eq!(
        graph.validate(&ledger, source).unwrap_err().to_string(),
        "invalid graph: part has no unit, heading or context owner"
    );
}

#[test]
fn unit_graph_wire_refuses_heading_parts_owned_by_units() {
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    let heading_id = graph.groups[0].part_ids[0].clone();
    graph.units[0].part_ids.insert(0, heading_id);
    assert_eq!(
        graph.validate(&ledger, source).unwrap_err().to_string(),
        "invalid graph: heading part cannot have a primary unit owner"
    );
}

#[test]
fn unit_graph_wire_context_is_derived_from_typed_ancestor_relations() {
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        graph.required_context("rows").unwrap(),
        ["part-1", "part-2"]
    );
    graph.groups[2].context_relations.remove(1);
    assert!(
        graph
            .validate(&ledger, include_str!("../fixtures/unit-graph-v1.txt"))
            .is_err()
    );
}

#[test]
fn unit_graph_wire_parent_parts_and_section_identity_are_exact() {
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    graph.groups[0].part_ids.pop();
    assert!(
        graph.validate(&ledger, source).is_err(),
        "parent drops child contribution"
    );
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    graph.groups[1].parent = Some("table".into());
    assert!(
        graph.validate(&ledger, source).is_err(),
        "table is not section ancestry"
    );
}

#[test]
fn unit_graph_wire_enforces_canonical_order_parentage_context_and_searchability() {
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let original = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    let mut changed = original.clone();
    changed.units.swap(0, 1);
    assert!(changed.validate(&ledger, source).is_err(), "unit order");
    let mut changed = original.clone();
    changed.groups.swap(0, 1);
    assert!(changed.validate(&ledger, source).is_err(), "group order");
    let mut changed = original.clone();
    changed.parts.swap(0, 1);
    assert!(changed.validate(&ledger, source).is_err(), "part order");
    let mut changed = original.clone();
    let mut later = changed.retrieval_views[0].clone();
    later.retrieval_view_id = "view-gap".into();
    later.chunk_id = "chunk-gap".into();
    later.memberships = vec![later.memberships[1].clone()];
    changed.retrieval_views.push(later);
    assert!(
        changed.validate(&ledger, source).is_ok(),
        "source-ordered second view"
    );
    changed.retrieval_views.swap(0, 1);
    assert!(changed.validate(&ledger, source).is_err(), "view order");
    let mut changed = original.clone();
    changed
        .groups
        .iter_mut()
        .find(|group| group.group_id == "row-group")
        .unwrap()
        .parent = Some("section".into());
    assert!(
        changed.validate(&ledger, source).is_err(),
        "wrong parent kind"
    );
    let mut changed = original.clone();
    changed
        .groups
        .iter_mut()
        .find(|group| group.group_id == "code-group")
        .unwrap()
        .kind = GroupKind::Table;
    assert!(
        changed.validate(&ledger, source).is_err(),
        "misplaced lead-in"
    );
    let mut changed = original.clone();
    changed.retrieval_views[0]
        .memberships
        .retain(|membership| membership.unit_id != "gap");
    assert!(
        changed.validate(&ledger, source).is_err(),
        "unsearchable part"
    );
    let mut changed = original;
    let duplicate = changed.units[0].part_ids[0].clone();
    changed.units[0].part_ids.push(duplicate);
    assert!(
        changed.validate(&ledger, source).is_err(),
        "duplicate part ID"
    );
}

#[test]
fn unit_graph_wire_checks_schema_profile_and_continuation() {
    use SplitMarker;
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    graph.descriptor.schema_version = "wrong".into();
    assert!(graph.validate(&ledger, source).is_err());
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    graph.descriptor.profile_name = "wrong".into();
    assert!(graph.validate(&ledger, source).is_err());
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    graph.units[0].split = SplitMarker::Continuation {
        ordinal: 2,
        total: 2,
    };
    assert!(graph.validate(&ledger, source).is_err());
}

#[test]
fn unit_graph_wire_accounts_a_valid_exclusion() {
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    let mut ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    graph.parts.retain(|part| part.part_id != "part-4");
    graph.units.retain(|unit| unit.unit_id != "gap");
    graph
        .groups
        .iter_mut()
        .find(|group| group.group_id == "section")
        .unwrap()
        .children
        .retain(|id| id != "gap");
    graph.retrieval_views[0]
        .memberships
        .retain(|member| member.unit_id != "gap");
    ledger.contributions.retain(|entry| entry.range.start != 28);
    let exclusion = Exclusion {
        range: SourceRange { start: 28, end: 32 },
        reason: "excluded synthetic gap".into(),
    };
    graph.exclusions.push(exclusion.clone());
    ledger.exclusions.push(exclusion);
    graph.descriptor.mapping_digest = Digest::of(&ledger.to_bytes().unwrap());
    assert!(
        graph.validate(&ledger, source).is_ok(),
        "{:?}",
        graph.validate(&ledger, source)
    );
    let mut invalid_graph = graph;
    let mut invalid_ledger = ledger;
    invalid_graph.exclusions[0].reason.clear();
    invalid_ledger.exclusions[0].reason.clear();
    invalid_graph.descriptor.mapping_digest = Digest::of(&invalid_ledger.to_bytes().unwrap());
    assert!(invalid_graph.validate(&invalid_ledger, source).is_err());
}

#[test]
fn unit_graph_wire_tiles_utf8_parts_only_at_character_boundaries() {
    let mut source = include_str!("../fixtures/unit-graph-v1.txt").to_owned();
    source.replace_range(28..32, "éxy");
    let mut graph = DeliveryGraph::from_bytes(graph_bytes()).unwrap();
    let mut ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let source_digest = Digest::of(source.as_bytes());
    graph.descriptor.original_markdown_digest = source_digest.clone();
    ledger.original_markdown_digest = source_digest;

    graph.parts[4].ranges[0].end = 30;
    graph.parts[4].mappings[0].derived_range = (0, 2);
    let second = Part {
        part_id: "part-split".into(),
        ranges: vec![SourceRange { start: 30, end: 32 }],
        mappings: vec![MappingContribution {
            unit_id: "canonical-4".into(),
            derived_range: (2, 4),
            mapping_mode: "exact".into(),
        }],
    };
    graph.parts.insert(5, second);
    graph.units[3].part_ids.insert(1, "part-split".into());
    for membership in graph
        .retrieval_views
        .iter_mut()
        .flat_map(|view| &mut view.memberships)
    {
        if membership.unit_id == "gap" {
            membership.primary_part_ids.push("part-split".into());
        }
    }
    ledger.contributions[4].range.end = 30;
    ledger.contributions[4].mapping.derived_range = (0, 2);
    ledger.contributions.insert(
        5,
        MappingEntry {
            range: SourceRange { start: 30, end: 32 },
            mapping: graph.parts[5].mappings[0].clone(),
        },
    );
    graph.descriptor.mapping_digest = Digest::of(&ledger.to_bytes().unwrap());
    assert!(graph.validate(&ledger, &source).is_ok());

    graph.parts[4].ranges[0].end = 29;
    graph.parts[4].mappings[0].derived_range = (0, 1);
    graph.parts[5].ranges[0].start = 29;
    graph.parts[5].mappings[0].derived_range = (1, 4);
    ledger.contributions[4].range.end = 29;
    ledger.contributions[4].mapping.derived_range = (0, 1);
    ledger.contributions[5].range.start = 29;
    ledger.contributions[5].mapping.derived_range = (1, 4);
    graph.descriptor.mapping_digest = Digest::of(&ledger.to_bytes().unwrap());
    assert!(
        graph.validate(&ledger, &source).is_err(),
        "a range cannot split the UTF-8 encoding of é"
    );
}
