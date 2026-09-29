use super::helpers::*;
use super::*;
use crate::unit_graph::types::SplitMarker;
use std::{collections::BTreeMap, slice};

#[test]
fn source_part_rejects_empty_empty_span_and_overlapping_origins_but_keeps_touching_ranges() {
    assert!(group_helpers::source_part(&source_unit(Vec::new()), 0).is_err());
    assert!(group_helpers::source_part(&source_unit(vec![(4, 4)]), 0).is_err());
    assert!(group_helpers::source_part(&source_unit(vec![(0, 4), (3, 7)]), 0).is_err());
    assert!(group_helpers::source_part(&source_unit(vec![(0, 4), (4, 7)]), 0).is_ok());
    assert!(group_helpers::source_part(&source_unit(vec![(0, 3), (4, 7)]), 0).is_ok());
}

#[test]
fn section_and_table_groups_retain_their_direct_unit_membership() {
    let markdown = concat!(
        "# Operations\n\nOverview.\n\n## Install\n\nSet target.\n\n",
        "| Setting | Value |\n|---|---|\n| mode | safe |\n| tier | core |\n"
    );
    let graph = graph_for(markdown);
    let section = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Section)
        .unwrap();
    let expected: BTreeSet<_> = graph
        .units
        .iter()
        .filter(|unit| {
            unit.section_id.as_deref() == section.section_id.as_deref()
                || unit.source_block_id == section.section_id.as_deref().unwrap_or_default()
        })
        .flat_map(|unit| {
            unit.parts
                .iter()
                .filter(|part| part.role == PartRole::Primary)
                .map(|part| part.part_id.as_str())
        })
        .collect();
    let actual: BTreeSet<_> = section
        .parts
        .iter()
        .filter(|part| part.role == PartRole::Primary)
        .map(|part| part.part_id.as_str())
        .collect();
    assert_eq!(actual, expected);
    let table = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Table)
        .unwrap();
    let table_units: Vec<_> = graph
        .units
        .iter()
        .filter(|unit| unit.kind == UnitKind::Table)
        .collect();
    assert!(!table_units.is_empty());
    assert!(
        table_units
            .iter()
            .all(|unit| table.children.contains(&unit.unit_id))
    );
    let mut profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.table_tokens = 1;
    let unpacked = graph_for_profile(markdown, profile);
    let row_group_ids: BTreeSet<_> = unpacked
        .groups
        .iter()
        .filter(|group| group.kind == GroupKind::Row)
        .map(|group| group.group_id.as_str())
        .collect();
    assert_eq!(row_group_ids.len(), 2);
    let unpacked_table = unpacked
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Table)
        .unwrap();
    assert!(
        row_group_ids
            .iter()
            .all(|id| unpacked_table.children.contains(&id.to_string()))
    );
    assert!(
        unpacked
            .units
            .iter()
            .filter(|unit| unit.kind == UnitKind::Row)
            .all(|unit| !unpacked_table.children.contains(&unit.unit_id))
    );
}

#[test]
fn page_group_contains_each_primary_unit_part_and_its_heading() {
    let graph = graph_for("# Page\n\nFirst.\n\n## Section\n\nSecond.\n");
    let page = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Page)
        .unwrap();
    let expected: BTreeSet<_> = graph
        .units
        .iter()
        .flat_map(|unit| {
            unit.parts
                .iter()
                .filter(|part| part.role == PartRole::Primary)
                .map(|part| part.part_id.as_str())
        })
        .chain(page.heading.iter().map(String::as_str))
        .collect();
    let actual: BTreeSet<_> = page
        .parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn each_revision_uses_its_matching_graph_input() {
    let first_markdown = "# Revision\n\nFirst body.\n";
    let second_markdown = "# Revision\n\nSecond body.\n";
    let first = canonicalize(CanonicalizeInput::new(first_markdown, "same.md")).unwrap();
    let second = canonicalize(CanonicalizeInput::new(second_markdown, "same.md")).unwrap();
    assert_eq!(first.document_id, second.document_id);
    assert_ne!(first.revision_id, second.revision_id);
    let inputs = [
        UnitGraphInput {
            document: &first,
            markdown: first_markdown,
            collection_id: "collection",
            source_namespace: "first",
        },
        UnitGraphInput {
            document: &second,
            markdown: second_markdown,
            collection_id: "collection",
            source_namespace: "second",
        },
    ];
    let scope = DedupScope {
        tenant_id: "tenant".into(),
        workspace_id: "collection".into(),
        authorized_revisions: [
            RevisionKey {
                document_id: first.document_id.clone(),
                revision_id: first.revision_id.clone(),
            },
            RevisionKey {
                document_id: second.document_id.clone(),
                revision_id: second.revision_id.clone(),
            },
        ]
        .into(),
    };
    let batch = unit_documents(
        &scope,
        &inputs,
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::V2Unit),
        &FixtureCounter,
    )
    .unwrap();
    let namespaces: BTreeSet<_> = batch
        .graphs
        .iter()
        .map(|graph| graph.descriptor.source_namespace.as_str())
        .collect();
    assert_eq!(namespaces, ["first", "second"].into());
}

#[test]
fn complex_group_graph_is_byte_pinned() {
    let markdown = concat!(
        "# Operations\n\nOverview.\n\n",
        "## Install\n\nSet the deployment target.\n\n",
        "| Setting | Value |\n|---|---|\n| mode | safe |\n\n",
        "1. Prepare.\n   - Check access.\n2. Deploy.\n\n",
        "```sh\nrun deploy\n```\n\n",
        "## Install\n\nRepeatable section.\n"
    );
    let graph = graph_for(markdown);
    let bytes = serialize_graph(&graph).unwrap();
    assert_eq!(
        digest(&bytes),
        "1647f3a45d35318a93aa0206239470bdc59406b8caf501cde4b35d39280dcd7d"
    );
}

#[test]
fn coverage_ledger_contains_only_sorted_primary_ranges() {
    let units = [
        delivery_unit("later", PartRole::Primary, 20),
        delivery_unit("context", PartRole::RequiredContext, 5),
        delivery_unit("earlier", PartRole::Primary, 10),
    ];
    let entries = ledger::coverage_ledger(&units);
    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.unit_id.as_str(), entry.range.start))
            .collect::<Vec<_>>(),
        [("earlier", 10), ("later", 20)]
    );
}

#[test]
fn mapping_artifact_checks_pairing_and_keeps_primary_unit_parts() {
    let units = [
        delivery_unit("owned", PartRole::Primary, 10),
        delivery_unit("context", PartRole::RequiredContext, 20),
    ];
    let artifact = ledger::mapping_artifact("source", &units, &[], &[]).unwrap();
    assert_eq!(artifact.schema_version, "maestro-unit-mapping/1");
    assert_eq!(artifact.contributions.len(), 1);
    assert_eq!(artifact.contributions[0].mapping.unit_id, "owned");
    let broken = DeliveryUnit {
        parts: vec![SourcePart {
            ranges: vec![SourceRange { start: 10, end: 11 }],
            mappings: Vec::new(),
            ..part("broken", PartRole::Primary, 10)
        }],
        ..units[0].clone()
    };
    assert!(ledger::mapping_artifact("source", &[broken], &[], &[]).is_err());
}

#[test]
fn exclusions_keep_noneligible_source_and_deduplicate_only_identical_entries() {
    let markdown = "---\ntitle: Guide\n---\n# Guide\n\nBody.\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "guide.md")).unwrap();
    let mapped = map_document(&document, markdown).unwrap();
    let baseline = ledger::exclusions(&document, &mapped, &[]).unwrap();
    let source = baseline.first().unwrap();
    let same = Exclusion {
        range: source.range,
        reason: source.reason.clone(),
    };
    let distinct = Exclusion {
        range: source.range,
        reason: "another_reason".into(),
    };
    let deduped = ledger::exclusions(&document, &mapped, &[same]).unwrap();
    assert_eq!(
        deduped
            .iter()
            .filter(|entry| entry.range == source.range && entry.reason == source.reason)
            .count(),
        1
    );
    let kept = ledger::exclusions(&document, &mapped, &[distinct]).unwrap();
    assert!(
        kept.iter()
            .any(|entry| entry.range == source.range && entry.reason == source.reason)
    );
    assert!(
        kept.iter()
            .any(|entry| entry.range == source.range && entry.reason == "another_reason")
    );
    assert!(baseline.iter().all(|entry| entry.reason != "eligible"));
}

#[test]
fn complete_ideas_membership_names_only_its_primary_source_parts_once() {
    let markdown = concat!(
        "# Operations\n\nOverview.\n\n",
        "## Install\n\nSet the deployment target.\n\n",
        "| Setting | Value |\n|---|---|\n| mode | safe |\n| tier | core |\n\n",
        "1. Prepare.\n   - Check access.\n2. Deploy.\n\n",
        "```sh\nrun deploy\n```\n"
    );
    let graph = graph_for_profile(markdown, UnitProfile::new(RankedUnit::CompleteIdeas));
    let primary_parts: BTreeMap<_, _> = graph
        .units
        .iter()
        .flat_map(|unit| {
            unit.parts
                .iter()
                .filter(|part| part.role == PartRole::Primary)
        })
        .map(|part| (part.part_id.as_str(), part))
        .collect();
    for membership in graph
        .retrieval_views
        .iter()
        .flat_map(|view| &view.memberships)
    {
        let ids: BTreeSet<_> = membership
            .primary_part_ids
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(ids.len(), membership.primary_part_ids.len());
        assert!(membership.primary_part_ids.iter().all(|part_id| {
            primary_parts.get(part_id.as_str()).is_some_and(|part| {
                part.ranges
                    .iter()
                    .any(|range| membership.primary_ranges.contains(range))
            })
        }));
    }
}

#[test]
fn complete_ideas_markup_is_excluded_as_source_chrome() {
    let markdown = concat!(
        "# Restart Link copied to clipboard\n\n",
        "Closed\n\n<!-- image -->\n\n",
        "Restart it. Copy Copied to clipboard\n\n",
        "Copy Copied to clipboard\n"
    );
    let document = canonicalize(CanonicalizeInput::new(markdown, "chrome.md")).unwrap();
    let mapped = map_document(&document, markdown).unwrap();
    let (unit_ids, exclusions) = ledger::chrome_markup(&document, markdown, &mapped).unwrap();
    assert_eq!(unit_ids.len(), 1);
    assert!(exclusions.iter().any(|exclusion| {
        exclusion.reason == "chrome_markup"
            && &markdown[exclusion.range.start..exclusion.range.end] == "<!-- image -->\n"
    }));
}

#[test]
fn cyclic_group_ancestry_is_rejected_before_traversal_repeats() {
    let unit = DeliveryUnit {
        unit_id: "unit".into(),
        kind: UnitKind::Block,
        source_block_id: "block".into(),
        section_id: None,
        heading_path: Vec::new(),
        parent_id: Some("cycle".into()),
        parts: Vec::new(),
        split: SplitMarker::Whole,
    };
    let groups = [Group {
        group_id: "cycle".into(),
        kind: GroupKind::Section,
        section_id: None,
        parent_id: Some("cycle".into()),
        children: Vec::new(),
        parts: Vec::new(),
        heading: None,
        family: FamilyKey {
            collection_id: "collection".into(),
            source_namespace: "synthetic".into(),
            path_segment: "page".into(),
            page_title: "page".into(),
            heading_path: Vec::new(),
            occurrence: 0,
        },
    }];
    let index = prepared::GraphIndex::new(slice::from_ref(&unit), &groups, &[]);
    assert!(prepared::unit_heading_parts_indexed(&unit, &index).is_err());
    assert!(prepared::unit_context_parts_indexed(&unit, &index).is_err());
}
