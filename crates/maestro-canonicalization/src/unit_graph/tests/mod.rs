//! Unit graph profile and delivery contract regressions.
use super::*;
use crate::hashing::digest;
use crate::{
    CanonicalizeInput, DedupScope, Error, RevisionKey, TokenCounter, WarningPolicy, canonicalize,
    unit_documents,
};
use crate::{chunk_mapping::map_document, chunk_split::MAX_TOKENS};
use std::{cell::Cell, collections::BTreeSet};

#[test]
fn ranked_rules_are_digest_pinned_without_changing_legacy_profiles() {
    let complete = UnitProfile::new(RankedUnit::CompleteIdeas);
    let v2 = UnitProfile::new(RankedUnit::V2Unit);
    assert_eq!(complete.name(), "mapped-structural-chunks/4");
    assert_eq!(complete.preparation_name(), "canonical-context-parts/v3");
    assert_ne!(
        complete.profile_digest().unwrap(),
        v2.profile_digest().unwrap()
    );
    assert!(complete.size_limits().all_within_verified_counter_cap());
}

#[test]
fn every_profile_limit_rejects_zero_and_values_above_counter_cap() {
    let maximum = MAX_TOKENS;
    let valid = UnitSizeLimits {
        section_tokens: maximum,
        table_tokens: maximum,
        row_tokens: maximum,
        procedure_tokens: maximum,
        code_tokens: maximum,
        paragraphs_tokens: maximum,
    };
    assert!(valid.all_within_verified_counter_cap());
    for limit in 0..6 {
        assert!(!set_limit(valid, limit, 0).all_within_verified_counter_cap());
        assert!(!set_limit(valid, limit, maximum + 1).all_within_verified_counter_cap());
    }
}

#[test]
fn unit_profile_refuses_limits_over_the_counter_cap_before_counting() {
    let markdown = "# T\n\nbody\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "limit.md")).unwrap();
    let scope = DedupScope {
        tenant_id: "tenant".into(),
        workspace_id: "collection".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into(),
    };
    let input = UnitGraphInput {
        document: &document,
        markdown,
        collection_id: "collection",
        source_namespace: "synthetic",
    };
    let mut profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.code_tokens = MAX_TOKENS + 1;
    let counter = Counter(Cell::new(0));
    let error =
        unit_documents(&scope, &[input], WarningPolicy::Preserve, profile, &counter).unwrap_err();
    assert!(
        matches!(error, UnitGraphError::InvalidInput(ref error) if error.0.contains("counter cap"))
    );
    assert_eq!(counter.0.get(), 0);
}

fn set_limit(mut limits: UnitSizeLimits, index: usize, value: usize) -> UnitSizeLimits {
    match index {
        0 => limits.section_tokens = value,
        1 => limits.table_tokens = value,
        2 => limits.row_tokens = value,
        3 => limits.procedure_tokens = value,
        4 => limits.code_tokens = value,
        _ => limits.paragraphs_tokens = value,
    }
    limits
}

#[test]
fn shared_wire_fixtures_round_trip_byte_for_byte() {
    let graph_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-kernel/tests/fixtures/unit-graph-v1.json"
    ));
    let graph_cas = graph_bytes.strip_suffix(b"\n").unwrap();
    let graph = serialization::parse_wire_graph(graph_cas).unwrap();
    assert_eq!(
        serialization::serialize_wire_graph(&graph).unwrap(),
        graph_cas
    );
    assert_eq!(
        digest(graph_cas),
        "8717210bb0b68d26265e7c2478d3a10c32932bc811bb068ba14f093672a2d01d"
    );
    let mut unknown_field = graph_cas.to_vec();
    unknown_field.splice(1..1, b"\"unknown\":null,".iter().copied());
    assert!(serialization::parse_wire_graph(&unknown_field).is_err());
    let descriptor_key = b"\"descriptor\":";
    let descriptor_start = graph_cas
        .windows(descriptor_key.len())
        .position(|window| window == descriptor_key)
        .unwrap()
        + descriptor_key.len();
    let units_key = b",\"units\":";
    let descriptor_end = descriptor_start
        + graph_cas[descriptor_start..]
            .windows(units_key.len())
            .position(|window| window == units_key)
            .unwrap();
    let descriptor = &graph_cas[descriptor_start..descriptor_end];
    let mut duplicate_json = b"{\"descriptor\":".to_vec();
    duplicate_json.extend_from_slice(descriptor);
    duplicate_json.extend_from_slice(b",\"descriptor\":");
    duplicate_json.extend_from_slice(descriptor);
    duplicate_json.push(b'}');
    let duplicate = serialization::parse_wire_graph(&duplicate_json).unwrap_err();
    assert!(duplicate.to_string().contains("duplicate field"));
    assert!(
        !String::from_utf8(serialization::serialize_wire_graph(&graph).unwrap())
            .unwrap()
            .contains("graph_digest")
    );

    let mapping_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-kernel/tests/fixtures/unit-mapping-v1.json"
    ));
    let mapping_cas = mapping_bytes.strip_suffix(b"\n").unwrap();
    let mapping = serialization::parse_wire_mapping(mapping_cas).unwrap();
    assert_eq!(
        serialization::serialize_wire_mapping(&mapping).unwrap(),
        mapping_cas
    );
    assert_eq!(
        digest(mapping_cas),
        "de4b7353da814a92f84c8ffcf8cd03a055d7dc8d770e62bc09b282a2431bb4dd"
    );
}

#[test]
fn unknown_ranked_rules_never_downgrade() {
    assert_eq!(
        RankedUnit::named("CompleteIdeas"),
        Some(RankedUnit::CompleteIdeas)
    );
    assert_eq!(RankedUnit::named("V2Unit"), Some(RankedUnit::V2Unit));
    assert!(RankedUnit::named("future-unit-rule").is_none());
}

#[test]
fn both_ranking_rules_keep_primary_delivery_graph_and_exact_source_ranges() {
    let markdown = concat!(
        "# Sample\n\nA UTF-8 paragraph: café.\n\n",
        "| Name | Value |\n|---|---|\n| row | 2 |\n\n",
        "1. Step one.\n2. Step two.\n\n```\ncode\n```\n"
    );
    let document = canonicalize(CanonicalizeInput::new(markdown, "guide.md")).unwrap();
    let scope = DedupScope {
        tenant_id: "tenant".into(),
        workspace_id: "collection".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into(),
    };
    let input = UnitGraphInput {
        document: &document,
        markdown,
        collection_id: "collection",
        source_namespace: "docs/product-a",
    };
    let counter = Counter(Cell::new(0));
    let complete = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::CompleteIdeas),
        &counter,
    )
    .unwrap();
    let v2 = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::V2Unit),
        &counter,
    )
    .unwrap();
    let left = &complete.graphs[0];
    let right = &v2.graphs[0];
    assert_eq!(left.units, right.units);
    assert_eq!(left.groups, right.groups);
    assert_ne!(
        left.retrieval_views[0].rank_policy,
        right.retrieval_views[0].rank_policy
    );
    let owned: Vec<_> = left
        .units
        .iter()
        .flat_map(|unit| &unit.parts)
        .filter(|part| part.role == PartRole::Primary)
        .flat_map(|part| &part.ranges)
        .map(|range| &markdown[range.start..range.end])
        .collect();
    assert!(owned.iter().any(|slice| slice.contains("café")));
    assert!(owned.iter().any(|slice| slice.contains("row")));
    assert!(
        left.groups
            .iter()
            .any(|group| group.kind == GroupKind::Table)
    );
    assert!(
        left.groups
            .iter()
            .any(|group| group.kind == GroupKind::Procedure)
    );
    let left_bytes = serialize_graph(left).unwrap();
    let right_bytes = serialize_graph(right).unwrap();
    assert_eq!(
        digest(&left_bytes),
        "9f14e9a6ea741a94aa50d428616f36c228a8b4a813f037359233b23ab8be3627"
    );
    assert_eq!(
        digest(&right_bytes),
        "5858d5a0902ec42b7505c49e4d4da70d6c07f731e6d55428f7a3992e8c5d6f53"
    );
    assert_eq!(left_bytes, serialize_graph(left).unwrap());
}

#[test]
fn producer_snapshot_matches_valid_source_fixture_and_contract_rules() {
    let markdown = include_str!("../../../tests/fixtures/unit-graph-v1.built.md");
    let document = canonicalize(CanonicalizeInput::new(markdown, "page.md")).unwrap();
    let scope = DedupScope {
        tenant_id: "tenant".into(),
        workspace_id: "collection".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into(),
    };
    let input = UnitGraphInput {
        document: &document,
        markdown,
        collection_id: "collection",
        source_namespace: "synthetic",
    };
    let batch = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::V2Unit),
        &FixtureCounter,
    )
    .unwrap();
    let graph = batch.graphs.first().unwrap();
    assert_producer_snapshot(graph);
    assert_wire_structure(graph);
    let repeated = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::V2Unit),
        &FixtureCounter,
    )
    .unwrap();
    assert_eq!(
        serialize_graph(graph).unwrap(),
        serialize_graph(repeated.graphs.first().unwrap()).unwrap()
    );
    assert_producer_structure(graph);
    assert_producer_contexts(graph, &document, markdown);
}

/// Compare deterministic producer bytes to the owned valid-source snapshot.
fn assert_producer_snapshot(graph: &DeliveryGraph) {
    let bytes = serialize_graph(graph).unwrap();
    let snapshot = include_bytes!("../../../tests/fixtures/unit-graph-v1.built.json");
    assert_eq!(bytes, snapshot.strip_suffix(b"\n").unwrap());
    assert_eq!(bytes, serialize_graph(graph).unwrap());
    assert!(!bytes.ends_with(b"\n"));
}

/// Check wire part references and direct group-part ownership.
fn assert_wire_structure(graph: &DeliveryGraph) {
    let bytes = serialize_graph(graph).unwrap();
    let wire = serialization::parse_wire_graph(&bytes).unwrap();
    let part_ids: BTreeSet<_> = wire
        .parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect();
    for unit in &wire.units {
        assert!(
            unit.part_ids
                .iter()
                .all(|part_id| part_ids.contains(part_id.as_str()))
        );
    }
    for group in &wire.groups {
        let expected: Vec<_> = group
            .heading
            .iter()
            .map(String::as_str)
            .chain(
                group
                    .context_relations
                    .iter()
                    .map(|relation| relation.part_id.as_str()),
            )
            .collect();
        assert_eq!(
            group
                .part_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

/// Check table, section-family and part-role producer invariants.
fn assert_producer_structure(graph: &DeliveryGraph) {
    assert!(graph.units.iter().any(|unit| unit.kind == UnitKind::Table));
    assert_eq!(
        graph
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Row)
            .count(),
        0
    );
    assert!(
        graph
            .groups
            .iter()
            .any(|group| group.kind == GroupKind::Table)
    );
    assert!(
        graph
            .context_relations
            .iter()
            .any(|relation| relation.kind == ContextRelation::HeaderToTable)
    );
    assert!(
        graph
            .units
            .iter()
            .flat_map(|unit| &unit.parts)
            .all(|part| part.role == PartRole::Primary)
    );
    assert!(
        !graph
            .context_relations
            .iter()
            .any(|relation| relation.kind == ContextRelation::CaptionToTable)
    );
    let repeated: Vec<_> = graph
        .groups
        .iter()
        .filter(|group| {
            group.kind == GroupKind::Section
                && group.family.heading_path == ["product guide", "installation"]
        })
        .collect();
    assert_eq!(repeated.len(), 2);
    assert_ne!(
        repeated.first().unwrap().family.occurrence,
        repeated.get(1).unwrap().family.occurrence
    );
}

/// Check procedure and code context relations and ancestor-derived prepared headings.
fn assert_producer_contexts(
    graph: &DeliveryGraph,
    document: &crate::CanonicalDocument,
    markdown: &str,
) {
    for (group_kind, unit_kind) in [
        (GroupKind::Procedure, UnitKind::Procedure),
        (GroupKind::Code, UnitKind::Code),
    ] {
        let group = graph
            .groups
            .iter()
            .find(|group| group.kind == group_kind)
            .unwrap();
        let unit = graph
            .units
            .iter()
            .find(|unit| unit.kind == unit_kind)
            .unwrap();
        assert!(group.children.contains(&unit.unit_id));
        let lead_in = graph
            .context_relations
            .iter()
            .find(|relation| {
                relation.group_id == group.group_id && relation.kind == ContextRelation::LeadIn
            })
            .unwrap();
        assert!(
            graph
                .retrieval_views
                .iter()
                .any(|view| view.memberships.iter().any(|membership| {
                    membership.unit_id == unit.unit_id
                        && membership.context_part_ids.contains(&lead_in.part_id)
                }))
        );
        if unit_kind == UnitKind::Code {
            let mapped = map_document(document, markdown).unwrap();
            let mapped_index = prepared::MappedTextIndex::new(&mapped);
            let graph_index =
                prepared::GraphIndex::new(&graph.units, &graph.groups, &graph.context_relations);
            let headings = prepared::unit_heading_parts_indexed(unit, &graph_index).unwrap();
            let contexts = prepared::unit_context_parts_indexed(unit, &graph_index).unwrap();
            let prepared =
                prepared::prepared_text(unit, &mapped_index, &contexts, &headings).unwrap();
            assert!(prepared.starts_with("Product guide / Installation"));
            assert!(prepared.contains("Where: Set the deployment target."));
        }
    }
}

mod c1;
mod c2;
mod c3;
mod c4;
mod c5;
mod helpers;
mod i1;
mod i2;
mod i3;
mod i4;
mod i5;
mod i6;
mod i7;
mod producer_rows;
mod producer_rows_more;

struct Counter(Cell<usize>);

impl TokenCounter for Counter {
    fn contract_id(&self) -> &'static str {
        "test/unit-graph"
    }
    fn verify(&self) -> Result<(), Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        Ok(input.chars().map(u32::from).collect())
    }
}

struct FixtureCounter;

impl TokenCounter for FixtureCounter {
    fn contract_id(&self) -> &'static str {
        "synthetic-counter/1"
    }
    fn verify(&self) -> Result<(), Error> {
        Ok(())
    }
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        Ok(input.chars().map(u32::from).collect())
    }
}
