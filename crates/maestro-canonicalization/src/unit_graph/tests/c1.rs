use super::*;

#[test]
fn packed_table_has_no_empty_row_groups() {
    let markdown = "| Name | Value |\n|---|---|\n| row | 2 |\n";
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
        UnitProfile::new(RankedUnit::V2Unit),
        &FixtureCounter,
    )
    .unwrap();
    let graph = &batch.graphs[0];
    assert!(graph.units.iter().any(|unit| unit.kind == UnitKind::Table));
    assert!(
        graph
            .groups
            .iter()
            .all(|group| group.kind != GroupKind::Row)
    );
}
