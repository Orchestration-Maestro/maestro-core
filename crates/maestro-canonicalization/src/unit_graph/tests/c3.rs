use super::*;

#[test]
fn nested_ordered_and_bullet_lists_build() {
    let markdown = "# T\n\nIntro.\n\n1. Step one.\n   - sub a\n   - sub b\n2. Step two.\n";
    let graph = build_graph(markdown).unwrap();
    assert_eq!(
        graph
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Procedure)
            .count(),
        1
    );
}

#[test]
fn code_inside_a_list_builds() {
    let markdown = "# T\n\nIntro.\n\n- item\n\n  ```sh\n  run\n  ```\n";
    let graph = build_graph(markdown).unwrap();
    let code = graph
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Code)
        .unwrap();
    let group = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Code)
        .unwrap();
    assert_eq!(code.parent_id.as_deref(), Some(group.group_id.as_str()));
}

#[test]
fn code_without_a_lead_in_builds() {
    let graph = build_graph("# T\n\n```\ncode\n```\n").unwrap();
    let code = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Code)
        .unwrap();
    assert!(!graph.context_relations.iter().any(|relation| {
        relation.group_id == code.group_id && relation.kind == ContextRelation::LeadIn
    }));
}

#[test]
fn table_inside_a_list_builds() {
    let markdown = "# T\n\nIntro.\n\n- item\n\n  | A | B |\n  |---|---|\n  | 1 | 2 |\n";
    let graph = build_graph(markdown).unwrap();
    let table = graph
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Table)
        .unwrap();
    let group = graph
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Table)
        .unwrap();
    assert_eq!(table.parent_id.as_deref(), Some(group.group_id.as_str()));
}

fn build_graph(markdown: &str) -> Result<DeliveryGraph, Error> {
    let document = canonicalize(CanonicalizeInput::new(markdown, "nested.md"))?;
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
    .map_err(|error| Error(error.to_string()))?;
    Ok(batch.graphs[0].clone())
}
