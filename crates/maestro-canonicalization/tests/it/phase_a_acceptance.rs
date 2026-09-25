//! Phase A acceptance: every source byte is accounted for and no content is silently hidden.
#![cfg(test)]
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, ValidationStatus, canonicalize,
};

fn document(md: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(md, "acceptance.md")).unwrap()
}

#[test]
fn valid_dialect_surplus_cells_remain_raw_without_inventing_headers() {
    let md = "| Version |\n|---|\n|9.0.22|not 9.0.21|\n";
    let doc = document(md);
    assert_ne!(doc.validation_status, ValidationStatus::Failed);
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.retrieval_text.contains("not 9.0.21"))
    );
    let raw = doc
        .blocks
        .iter()
        .find(|block| serde_json::to_value(&block.block_type).unwrap() == "raw")
        .unwrap();
    assert_eq!(
        raw.retrieval_text,
        &md[raw.source_spans[0].start..raw.source_spans[0].end]
    );
    assert!(
        doc.warnings
            .iter()
            .any(|finding| finding.code == "source_fallback"
                && finding.block_id.as_ref() == Some(&raw.block_id))
    );
}

#[test]
fn valid_duplicate_definitions_preserve_both_destinations() {
    for md in [
        "[x]: https://first.test\n[x]: https://second.test\n\n[go][x]\n",
        "> [x]: https://first.test\n> [x]: https://second.test\n>\n> [go][x]\n",
    ] {
        let doc = document(md);
        assert_ne!(doc.validation_status, ValidationStatus::Failed);
        assert!(
            doc.blocks
                .iter()
                .any(|block| block.retrieval_text.contains("https://second.test"))
        );
        assert_eq!(doc.links[0].destination, "https://first.test");
    }
}

#[test]
fn every_original_byte_has_explicit_nonoverlapping_accounting() {
    let md = concat!(
        "---\ntitle: Exemple\n---\n# Étapes 🦀\n\n",
        "Ne pas modifier `repoName` en 9.0.22.\n\n<div>not hidden</div>\n"
    );
    let doc = document(md);
    let value = serde_json::to_value(&doc).unwrap();
    let accounting = value["source_accounting"]
        .as_array()
        .expect("explicit source ledger");
    let mut cursor = 0;
    for item in accounting {
        let start = usize::try_from(item["source_span"]["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(item["source_span"]["end"].as_u64().unwrap()).unwrap();
        assert_eq!(start, cursor);
        assert!(
            end > start
                && end <= md.len()
                && md.is_char_boundary(start)
                && md.is_char_boundary(end)
        );
        assert_ne!(item["role"], "unaccounted");
        cursor = end;
    }
    assert_eq!(cursor, md.len());
}

#[test]
fn meaning_sensitive_text_and_source_quotes_are_separate() {
    let md = "Ne **pas** changer `repoName`: 20 ms, version 9.0.22. Café e\u{301} 🦀.\n";
    let doc = document(md);
    assert_eq!(
        doc.blocks[0].retrieval_text,
        "Ne pas changer repoName: 20 ms, version 9.0.22. Café e\u{301} 🦀."
    );
    let span = doc.blocks[0].source_spans[0];
    assert_eq!(&md[span.start..span.end], md);
    for changed in [
        md.replace("pas", ""),
        md.replace("9.0.22", "9.0.21"),
        md.replace("repoName", "reponame"),
        md.replace("20 ms", "20 s"),
    ] {
        let other = document(&changed);
        assert_ne!(doc.content_hash, other.content_hash);
        assert_ne!(doc.revision_id, other.revision_id);
        assert_ne!(doc.blocks[0].retrieval_text, other.blocks[0].retrieval_text);
    }
}

#[test]
fn parser_omitted_heading_attributes_stay_raw_without_changing_the_title() {
    use maestro_canonicalization::{BlockType, SourceRole};
    for (md, title, retained) in [
        ("# Version {9}\n", "Version", "{9}"),
        ("# H {#repoName #last}\n", "H", "{#repoName #last}"),
        ("> # H {#repoName #last}\n", "H", "{#repoName #last}"),
        ("- # H {a .x #id}\n", "H", "{a .x #id}"),
        ("H {9}\n===\n", "H", "{9}"),
    ] {
        let doc = document(md);
        assert_ne!(doc.validation_status, ValidationStatus::Failed, "{md:?}");
        assert_eq!(doc.sections[0].title, title, "{md:?}");
        let raw = doc
            .blocks
            .iter()
            .find(|block| {
                block.block_type == BlockType::Raw && block.retrieval_text.contains(retained)
            })
            .expect("discarded attributes retained raw");
        assert!(
            raw.source_spans
                .iter()
                .any(|span| md[span.start..span.end] == raw.retrieval_text)
        );
        assert!(
            doc.source_accounting
                .iter()
                .any(|part| part.block_id.as_ref() == Some(&raw.block_id)
                    && part.role == SourceRole::Unsupported)
        );
        assert!(
            doc.warnings
                .iter()
                .any(|finding| finding.code == "source_fallback"
                    && finding.block_id.as_ref() == Some(&raw.block_id))
        );
    }
}

#[test]
fn html_only_document_retains_unsupported_content_with_a_warning() {
    let md = "<div>not 9.0.22</div>\n";
    let doc = document(md);
    assert_eq!(doc.validation_status, ValidationStatus::ValidWithWarnings);
    assert_eq!(doc.blocks[0].retrieval_text, md);
    assert!(
        doc.warnings
            .iter()
            .any(|finding| finding.code == "raw_html")
    );
}

#[test]
fn nul_replacement_is_visible_and_original_quotes_remain_exact() {
    let md = "Do not replace \0 in repoName 9.0.22.\n";
    let doc = document(md);
    assert_ne!(doc.validation_status, ValidationStatus::Failed);
    assert!(doc.warnings.iter().any(|finding| {
        finding.code == "extraction_artifact"
            && finding
                .source_spans
                .iter()
                .any(|span| &md[span.start..span.end] == "\0")
    }));
    let span = doc.blocks[0].source_spans[0];
    assert_eq!(&md[span.start..span.end], md);
}

#[test]
fn source_metadata_named_like_run_metadata_still_versions_provenance() {
    let mut input = CanonicalizeInput::new("Body\n", "source.md");
    input
        .metadata
        .extra
        .insert("run_id".into(), serde_json::json!("source-one"));
    let first = canonicalize(input.clone()).unwrap();
    input
        .metadata
        .extra
        .insert("run_id".into(), serde_json::json!("source-two"));
    let second = canonicalize(input).unwrap();
    assert_eq!(first.document_id, second.document_id);
    assert_eq!(first.content_hash, second.content_hash);
    assert_ne!(first.revision_id, second.revision_id);
    assert_eq!(first.source_metadata.extra["run_id"], "source-one");
    assert_eq!(second.source_metadata.extra["run_id"], "source-two");
}

#[test]
fn heading_free_text_is_supported_without_inferred_headings() {
    let doc = document("Ordinary prose.\n\nAnother paragraph.\n");
    assert_ne!(doc.validation_status, ValidationStatus::Failed);
    assert!(doc.sections.is_empty());
    assert!(
        doc.blocks
            .iter()
            .all(|block| block.parent_section_id.is_none() && block.heading_path.is_empty())
    );
}
