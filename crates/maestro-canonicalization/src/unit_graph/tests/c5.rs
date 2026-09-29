#[cfg(not(debug_assertions))]
use super::*;

#[cfg(not(debug_assertions))]
#[test]
fn thousand_row_table_builds_in_under_half_a_second() {
    use std::time::{Duration, Instant};

    let mut markdown = String::from("| Key | Value |\n|---|---|\n");
    for index in 0..1_000 {
        use std::fmt::Write;
        writeln!(&mut markdown, "| key-{index} | value-{index} |").unwrap();
    }
    let document = canonicalize(CanonicalizeInput::new(&markdown, "table.md")).unwrap();
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
        markdown: &markdown,
        collection_id: "collection",
        source_namespace: "synthetic",
    };
    let start = Instant::now();
    let batch = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::V2Unit),
        &FixtureCounter,
    )
    .unwrap();
    assert_eq!(
        batch.graphs[0]
            .units
            .iter()
            .filter(|unit| unit.kind == UnitKind::Row)
            .count(),
        1_000
    );
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "elapsed: {:?}",
        start.elapsed()
    );
}
