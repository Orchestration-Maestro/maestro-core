use super::*;

#[test]
fn complete_ideas_builds_the_producer_snapshot_with_arm_independent_units() {
    let markdown = include_str!("../../../tests/fixtures/unit-graph-v1.built.md");
    let unit_graph = build_snapshot(markdown, RankedUnit::V2Unit).unwrap();
    let ideas_graph = build_snapshot(markdown, RankedUnit::CompleteIdeas).unwrap();
    assert_eq!(unit_graph.units, ideas_graph.units);
    assert!(
        ideas_graph
            .exclusions
            .iter()
            .any(|exclusion| exclusion.reason == "chrome_markup")
    );
}

fn build_snapshot(markdown: &str, ranked_unit: RankedUnit) -> Result<DeliveryGraph, Error> {
    let document = canonicalize(CanonicalizeInput::new(markdown, "page.md"))?;
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
        UnitProfile::new(ranked_unit),
        &FixtureCounter,
    )
    .map_err(|error| Error(error.to_string()))?;
    Ok(batch.graphs[0].clone())
}
