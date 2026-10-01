use super::*;

#[test]
fn section_family_occurrence_ignores_rows_and_version_directory() {
    let first = "# Product\n\n## S\n\n| A | B |\n|---|---|\n| x | y |\n\n## S\n\ntext\n";
    let second =
        "# Product\n\n## S\n\n| A | B |\n|---|---|\n| x | y |\n| z | w |\n\n## S\n\ntext\n";
    let first_family = second_section_family(first, "docs/v1/page.md");
    let second_family = second_section_family(second, "docs/v2/page.md");
    assert_eq!(first_family, second_family);
}

fn second_section_family(markdown: &str, path: &str) -> FamilyKey {
    let mut canonical_input = CanonicalizeInput::new(markdown, path);
    canonical_input.metadata.source_reference = Some(path.to_owned());
    canonical_input.metadata.title = Some(
        if path.contains("/v1/") {
            "Release one"
        } else {
            "Release two"
        }
        .into(),
    );
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
        source_namespace: "synthetic",
    };
    let profile = UnitProfile {
        ranked_unit: RankedUnit::V2Unit,
        size_limits: UnitSizeLimits {
            table_tokens: 1,
            ..UnitProfile::new(RankedUnit::V2Unit).size_limits
        },
    };
    let batch = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        profile,
        &FixtureCounter,
    )
    .unwrap();
    batch.graphs[0]
        .groups
        .iter()
        .filter(|group| {
            group.kind == GroupKind::Section && group.family.heading_path == ["product", "s"]
        })
        .nth(1)
        .unwrap()
        .family
        .clone()
}
