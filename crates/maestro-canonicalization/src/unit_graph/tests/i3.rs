use super::*;

#[test]
fn empty_namespace_fallback_is_stable_for_document_revisions() {
    let first = build_graph("# Guide\n\nfirst.\n");
    let second = build_graph("# Guide\n\nsecond.\n");
    assert_eq!(first.descriptor.source_namespace, "local:stable-document");
    assert_eq!(second.descriptor.source_namespace, "local:stable-document");
    let first_family = first
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Section)
        .unwrap();
    let second_family = second
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Section)
        .unwrap();
    assert_eq!(
        first_family.family.source_namespace,
        second_family.family.source_namespace
    );
}

fn build_graph(markdown: &str) -> DeliveryGraph {
    let mut canonical_input = CanonicalizeInput::new(markdown, "docs/v1/guide.md");
    canonical_input.document_id = Some("stable-document");
    let document = canonicalize(canonical_input).unwrap();
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
        source_namespace: "",
    };
    let batch = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::V2Unit),
        &FixtureCounter,
    )
    .unwrap();
    batch.graphs[0].clone()
}
