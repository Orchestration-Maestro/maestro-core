use super::*;

#[test]
fn headings_are_group_parts_and_never_delivery_units() {
    let markdown = "# Product\n\nBody.\n";
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
    let graph = &batch.graphs[0];
    assert!(
        graph
            .units
            .iter()
            .all(|unit| unit.kind != UnitKind::Section)
    );
    let section = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Section)
        .unwrap();
    let heading = section.heading.as_ref().unwrap();
    assert!(
        section
            .parts
            .iter()
            .any(|part| part.part_id == *heading && part.role == PartRole::RequiredContext)
    );
    assert!(
        graph
            .units
            .iter()
            .all(|unit| unit.parts.iter().all(|part| part.part_id != *heading))
    );
}
