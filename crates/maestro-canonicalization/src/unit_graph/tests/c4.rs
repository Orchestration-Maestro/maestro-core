use super::*;

const TWO_CELL_TABLE: &str = "| Column A | Column B |\n|---|---|\n| alpha | beta |\n";

#[test]
fn packed_table_prepares_cells_in_source_order() {
    let (document, graph) = build_graph(TWO_CELL_TABLE, UnitProfile::new(RankedUnit::V2Unit));
    let mapped = map_document(&document, TWO_CELL_TABLE).unwrap();
    let mapped_index = prepared::MappedTextIndex::new(&mapped);
    let graph_index =
        prepared::GraphIndex::new(&graph.units, &graph.groups, &graph.context_relations);
    let table = graph
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Table)
        .unwrap();
    let contexts = prepared::unit_context_parts_indexed(table, &graph_index).unwrap();
    let headings = prepared::unit_heading_parts_indexed(table, &graph_index).unwrap();
    let text = prepared::prepared_text(table, &mapped_index, &contexts, &headings).unwrap();
    assert!(text.find("alpha").unwrap() < text.find("beta").unwrap());
}

#[test]
fn unpacked_row_prepares_header_context_and_cells_in_source_order() {
    let mut profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.table_tokens = 1;
    let (document, graph) = build_graph(TWO_CELL_TABLE, profile);
    let mapped = map_document(&document, TWO_CELL_TABLE).unwrap();
    let mapped_index = prepared::MappedTextIndex::new(&mapped);
    let graph_index =
        prepared::GraphIndex::new(&graph.units, &graph.groups, &graph.context_relations);
    let row = graph
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Row)
        .unwrap();
    let contexts = prepared::unit_context_parts_indexed(row, &graph_index).unwrap();
    let headings = prepared::unit_heading_parts_indexed(row, &graph_index).unwrap();
    let text = prepared::prepared_text(row, &mapped_index, &contexts, &headings).unwrap();
    assert!(text.find("Column A").unwrap() < text.find("Column B").unwrap());
    assert!(text.find("alpha").unwrap() < text.find("beta").unwrap());
}

fn build_graph(markdown: &str, profile: UnitProfile) -> (crate::CanonicalDocument, DeliveryGraph) {
    let document = canonicalize(CanonicalizeInput::new(markdown, "table.md")).unwrap();
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
    )
    .unwrap();
    let graph = batch.graphs[0].clone();
    drop(batch);
    (document, graph)
}
