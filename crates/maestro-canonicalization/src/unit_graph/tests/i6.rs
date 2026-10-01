use super::*;

#[test]
fn complete_ideas_accepts_a_large_paragraph_while_v2_refuses_typed() {
    let markdown = format!("# Size\n\n{}\n", "word ".repeat(700));
    let complete = build_graph(&markdown, RankedUnit::CompleteIdeas);
    assert!(
        complete.is_ok(),
        "CompleteIdeas refused page: {}",
        complete.unwrap_err()
    );

    let v2 = build_graph(&markdown, RankedUnit::V2Unit);
    assert!(matches!(
        v2,
        Err(UnitGraphError::OversizedUnitRefusal {
            unit_kind: UnitKind::Paragraphs,
            ..
        })
    ));
}

fn build_graph(markdown: &str, ranked_unit: RankedUnit) -> Result<(), UnitGraphError> {
    let document = canonicalize(CanonicalizeInput::new(markdown, "size.md")).unwrap();
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
    unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(ranked_unit),
        &FixtureCounter,
    )
    .map(|_| ())
}
