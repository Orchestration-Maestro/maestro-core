use super::helpers::*;
use super::*;
use crate::{content::BlockType, unit_graph::types::ContextRelationRecord};
use std::error::Error as StdError;

#[test]
fn unit_graph_mapping_validation_rejects_schema_and_source_gaps() {
    let markdown = "# Page\n\nA sufficiently long body.\n";
    let mut graph = graph_for(markdown);
    let mut mapping = ledger::mapping_artifact(
        &graph.descriptor.original_markdown_digest,
        &graph.units,
        &graph.groups,
        &graph.exclusions,
    )
    .unwrap();
    assert!(validation::validate_graph(&graph, &mapping, markdown).is_ok());
    let mut bad_schema = mapping.clone();
    bad_schema.schema_version = "future-schema".into();
    let mut bad_schema_graph = graph.clone();
    bad_schema_graph.descriptor.mapping_digest = digest(&serialize_mapping(&bad_schema).unwrap());
    assert!(validation::validate_graph(&bad_schema_graph, &bad_schema, markdown).is_err());

    let (part_id, range_index) = graph
        .units
        .iter()
        .flat_map(|unit| &unit.parts)
        .find_map(|part| {
            part.ranges
                .iter()
                .position(|range| range.end - range.start > 1)
                .map(|index| (part.part_id.clone(), index))
        })
        .unwrap();
    for part in graph
        .units
        .iter_mut()
        .flat_map(|unit| &mut unit.parts)
        .chain(graph.groups.iter_mut().flat_map(|group| &mut group.parts))
        .filter(|part| part.part_id == part_id)
    {
        part.ranges[range_index].start += 1;
    }
    mapping = ledger::mapping_artifact(
        &graph.descriptor.original_markdown_digest,
        &graph.units,
        &graph.groups,
        &graph.exclusions,
    )
    .unwrap();
    graph.descriptor.mapping_digest = digest(&serialize_mapping(&mapping).unwrap());
    assert!(validation::validate_graph(&graph, &mapping, markdown).is_err());
}

#[test]
fn mapping_validation_includes_only_primary_unit_parts() {
    let markdown = "# Page\n\nA body.\n";
    let mut graph = graph_for(markdown);
    graph.units[0].parts.push(SourcePart {
        part_id: "nonprimary-unit-part".into(),
        role: PartRole::RequiredContext,
        ordinal: 99,
        ranges: vec![SourceRange { start: 8, end: 9 }],
        mappings: vec![MappingContribution {
            unit_id: "context-only".into(),
            derived_range: (0, 1),
            mapping_mode: "exact".into(),
        }],
    });
    let mapping = ledger::mapping_artifact(
        &graph.descriptor.original_markdown_digest,
        &graph.units,
        &graph.groups,
        &graph.exclusions,
    )
    .unwrap();
    assert!(validation::validate_graph(&graph, &mapping, markdown).is_ok());
}

#[test]
fn unit_graph_display_and_source_errors_keep_their_contracts() {
    let display = UnitGraphError::OversizedUnitRefusal {
        unit_id: "unit-1".into(),
        unit_kind: UnitKind::Table,
    };
    assert_eq!(
        display.to_string(),
        "oversized_unit_refusal: unit-1 (Table)"
    );
    let invalid = UnitGraphError::InvalidInput(Error("invalid input".into()));
    assert!(StdError::source(&invalid).is_some());
}

#[test]
fn graph_digest_is_the_hash_of_its_exact_serialized_bytes() {
    let graph = graph_for("# Page\n\nBody text.\n");
    assert_eq!(
        serialization::graph_digest(&graph).unwrap(),
        digest(&serialize_graph(&graph).unwrap())
    );
}

#[test]
fn prepared_graph_index_returns_and_expands_units() {
    let graph = graph_for("# Page\n\n| Name | Value |\n|---|---|\n| mode | safe |\n");
    let index = prepared::GraphIndex::new(&graph.units, &graph.groups, &graph.context_relations);
    assert!(
        graph
            .units
            .iter()
            .all(|unit| index.unit(&unit.unit_id).is_some())
    );
    let table = graph
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Table)
        .unwrap();
    assert!(
        !prepared::unit_all_context_parts_indexed(table, &index)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn structural_group_ids_are_distinct_stable_sha256_identities() {
    let markdown = "# Alpha\n\n## Beta\n\n| A | B |\n|---|---|\n| x | y |\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "ids.md")).unwrap();
    let input = UnitGraphInput {
        document: &document,
        markdown,
        collection_id: "collection",
        source_namespace: "synthetic",
    };
    let first = group_helpers::section_group_id(&input, &document.sections[0].section_id).unwrap();
    let second = group_helpers::section_group_id(&input, &document.sections[1].section_id).unwrap();
    let table = document
        .blocks
        .iter()
        .find(|block| block.block_type == BlockType::Table)
        .unwrap();
    let table_id = group_helpers::structural_group_id(&input, table).unwrap();
    let digest = group_helpers::sha(&("stable", 7)).unwrap();
    assert!(first.starts_with("section-") && first.len() > 8);
    assert!(second.starts_with("section-") && second.len() > 8);
    assert_ne!(first, second);
    assert!(table_id.starts_with("group-") && table_id.len() > 6);
    assert_eq!(digest.len(), 64);
}

#[test]
fn context_relations_are_source_ordered_with_group_local_ordinals() {
    let parts: Vec<_> = [10, 20, 30]
        .into_iter()
        .enumerate()
        .map(|(ordinal, start)| SourcePart {
            part_id: format!("part-{ordinal}"),
            role: PartRole::RequiredContext,
            ordinal,
            ranges: vec![SourceRange {
                start,
                end: start + 1,
            }],
            mappings: Vec::new(),
        })
        .collect();
    let groups = [Group {
        group_id: "group".into(),
        kind: GroupKind::Table,
        section_id: None,
        parent_id: None,
        children: Vec::new(),
        parts,
        heading: None,
        family: FamilyKey {
            collection_id: "collection".into(),
            source_namespace: "synthetic".into(),
            path_segment: "table".into(),
            page_title: "page".into(),
            heading_path: Vec::new(),
            occurrence: 0,
        },
    }];
    let mut relations = [30, 20, 10]
        .into_iter()
        .map(|start| ContextRelationRecord {
            group_id: "group".into(),
            kind: ContextRelation::HeaderToTable,
            part_id: format!("part-{}", (start / 10) - 1),
            ordinal: usize::MAX,
        })
        .collect::<Vec<_>>();
    group_links::order_context_relations(&groups, &mut relations);
    assert_eq!(
        relations
            .iter()
            .map(|relation| (relation.part_id.as_str(), relation.ordinal))
            .collect::<Vec<_>>(),
        [("part-0", 0), ("part-1", 1), ("part-2", 2)]
    );
}

#[test]
fn nested_lists_fold_into_their_enclosing_list_only() {
    let markdown = "# T\n\n- parent\n  - child\n\n> - quoted\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "lists.md")).unwrap();
    let mapped = map_document(&document, markdown).unwrap();
    let input = UnitGraphInput {
        document: &document,
        markdown,
        collection_id: "collection",
        source_namespace: "synthetic",
    };
    let index = group_helpers::SourceIndex::new(&input, &mapped);
    let lists: Vec<_> = document
        .blocks
        .iter()
        .filter(|block| block.block_type == BlockType::List)
        .collect();
    assert_eq!(lists.len(), 3);
    assert!(!index.nested_list(&lists[0].block_id));
    assert!(index.nested_list(&lists[1].block_id));
    assert!(!index.nested_list(&lists[2].block_id));
}
