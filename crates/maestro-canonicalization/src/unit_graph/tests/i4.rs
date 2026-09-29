use super::*;

#[test]
fn code_after_a_table_does_not_reuse_a_distant_paragraph_as_lead_in() {
    let markdown = "# T\n\nLead para.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "far-lead.md")).unwrap();
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
    let code = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Code)
        .unwrap();
    assert!(!graph.context_relations.iter().any(|relation| {
        relation.group_id == code.group_id && relation.kind == ContextRelation::LeadIn
    }));
}
