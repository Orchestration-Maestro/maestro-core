use super::*;
use std::fmt::Write as _;

#[test]
fn unit_size_limit_accepts_below_and_at_the_boundary_and_refuses_above() {
    let mut profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.paragraphs_tokens = 20;
    assert!(build_graph(&paragraph(16), profile).is_ok());
    assert!(build_graph(&paragraph(17), profile).is_ok());
    assert!(matches!(
        build_graph(&paragraph(18), profile),
        Err(UnitGraphError::OversizedUnitRefusal { .. })
    ));
}

#[test]
fn small_table_packs_below_at_and_above_its_limit() {
    let markdown = "# T\n\n| A | B |\n|---|---|\n| x | y |\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "sizes.md")).unwrap();
    let mapped = map_document(&document, markdown).unwrap();
    let mapped_index = prepared::MappedTextIndex::new(&mapped);
    let base = build_graph_for_profile(markdown, UnitProfile::new(RankedUnit::V2Unit)).unwrap();
    let table = base
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Table)
        .unwrap();
    let count = FixtureCounter
        .token_ids(&prepared::prepared_text(table, &mapped_index, &[], &[]).unwrap())
        .unwrap()
        .len();
    for (limit, packed) in [(count - 1, false), (count, true), (count + 1, true)] {
        let mut profile = UnitProfile::new(RankedUnit::V2Unit);
        profile.size_limits.table_tokens = limit;
        let graph = build_graph_for_profile(markdown, profile).unwrap();
        assert_eq!(
            graph.units.iter().any(|unit| unit.kind == UnitKind::Table),
            packed,
            "table limit {limit}"
        );
    }
}

#[test]
fn multiple_small_tables_pack_with_primary_parts() {
    let markdown = concat!(
        "# T\n\n| A | B |\n|---|---|\n| x | y |\n| p | q |\n\n",
        "- review\n\n  | C | D |\n  |---|---|\n  | z | w |\n"
    );
    let graph = build_graph_for_profile(markdown, UnitProfile::new(RankedUnit::V2Unit)).unwrap();
    let tables: Vec<_> = graph
        .units
        .iter()
        .filter(|unit| unit.kind == UnitKind::Table)
        .collect();
    assert_eq!(tables.len(), 2);
    assert!(tables.iter().all(|table| !table.parts.is_empty()));
}

#[test]
fn oversized_rows_steps_and_code_return_their_typed_kind() {
    let mut profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.row_tokens = 1;
    profile.size_limits.table_tokens = 1;
    assert_kind(
        &build_graph("# T\n\n| A | B |\n|---|---|\n| alpha | beta |\n", profile),
        UnitKind::Row,
    );

    profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.procedure_tokens = 1;
    assert_kind(
        &build_graph("# T\n\n- a long step\n", profile),
        UnitKind::Procedure,
    );

    profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.code_tokens = 1;
    assert_kind(
        &build_graph("# T\n\n```\na long code line\n```\n", profile),
        UnitKind::Code,
    );
}

#[test]
fn mapped_ranges_stay_on_utf8_boundaries() {
    let markdown = "# Café\n\nA naïve résumé.\n";
    let graph = build_graph_for_profile(markdown, UnitProfile::new(RankedUnit::V2Unit)).unwrap();
    for range in graph
        .coverage
        .iter()
        .map(|entry| entry.range)
        .chain(graph.exclusions.iter().map(|entry| entry.range))
    {
        assert!(markdown.is_char_boundary(range.start));
        assert!(markdown.is_char_boundary(range.end));
    }
}

#[test]
fn far_down_row_survives_in_an_unpacked_table() {
    let mut rows = String::new();
    for index in 0..80 {
        writeln!(rows, "| row {index} | value {index} |")
            .expect("writing to a String is infallible");
    }
    let markdown = format!("# T\n\n| Name | Value |\n|---|---|\n{rows}");
    let mut profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.table_tokens = 1;
    let graph = build_graph_for_profile(&markdown, profile).unwrap();
    let tail_start = markdown.rfind("row 79").unwrap();
    assert!(graph.groups.iter().any(|group| {
        group.kind == GroupKind::Row
            && group
                .parts
                .iter()
                .any(|part| part.ranges.iter().any(|range| range.start >= tail_start))
    }));
}

#[test]
fn complete_ideas_view_can_span_multiple_delivery_units() {
    let markdown = "# T\n\nFirst paragraph.\n\nSecond paragraph.\n";
    let graph =
        build_graph_for_profile(markdown, UnitProfile::new(RankedUnit::CompleteIdeas)).unwrap();
    assert!(
        graph
            .retrieval_views
            .iter()
            .any(|view| view.memberships.len() >= 2)
    );
}

fn paragraph(body_len: usize) -> String {
    format!("# H\n\n{}\n", "x".repeat(body_len))
}

fn assert_kind(result: &Result<(), UnitGraphError>, expected: UnitKind) {
    assert!(matches!(
        result,
        Err(UnitGraphError::OversizedUnitRefusal { unit_kind, .. }) if *unit_kind == expected
    ));
}

fn build_graph(markdown: &str, profile: UnitProfile) -> Result<(), UnitGraphError> {
    build_graph_for_profile(markdown, profile).map(|_| ())
}

fn build_graph_for_profile(
    markdown: &str,
    profile: UnitProfile,
) -> Result<DeliveryGraph, UnitGraphError> {
    let document = canonicalize(CanonicalizeInput::new(markdown, "sizes.md")).unwrap();
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
        profile,
        &FixtureCounter,
    )?;
    Ok(batch.graphs[0].clone())
}
