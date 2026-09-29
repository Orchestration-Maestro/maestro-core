use super::*;

#[test]
fn second_top_level_section_uses_its_heading_without_page_duplication() {
    let markdown = "# Alpha\n\nfirst.\n\n# Beta\n\nsecond.\n";
    let (document, graph) = build_graph(markdown);
    let mapped = map_document(&document, markdown).unwrap();
    let mapped_index = prepared::MappedTextIndex::new(&mapped);
    let graph_index =
        prepared::GraphIndex::new(&graph.units, &graph.groups, &graph.context_relations);
    let unit = graph
        .units
        .iter()
        .find(|unit| {
            unit.kind == UnitKind::Paragraphs
                && unit.parts[0].mappings.iter().any(|mapping| {
                    mapped
                        .units
                        .iter()
                        .any(|source| source.unit_id == mapping.unit_id && source.text == "second.")
                })
        })
        .unwrap();
    let headings = prepared::unit_heading_parts_indexed(unit, &graph_index).unwrap();
    let text = prepared::prepared_text(unit, &mapped_index, &[], &headings).unwrap();
    assert_eq!(text, "Alpha / Beta\n\nsecond.");
}

fn build_graph(markdown: &str) -> (crate::CanonicalDocument, DeliveryGraph) {
    let document = canonicalize(CanonicalizeInput::new(markdown, "headings.md")).unwrap();
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
    let graph = batch.graphs[0].clone();
    drop(batch);
    (document, graph)
}
