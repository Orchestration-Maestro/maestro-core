//! The canonical document's contract: structure, spans and provenance as the source gives them.
#![cfg(test)]
use maestro_canonicalization::{
    AssetStatus, BlockAttributes, BlockType, CanonicalDocument, CanonicalizeInput, CodeKind,
    ExtractorBlock, OriginalLocation, Severity, SourceSpan, ValidationStatus, canonicalize,
};
use serde_json::json;

fn document(markdown: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(markdown, "fixtures/source.md")).unwrap()
}

#[test]
fn repeated_and_skipped_headings_keep_distinct_sections() {
    let doc = document("intro\n\n# Install\n\n### Same\n\nfirst\n\n### Same\n\nsecond\n\n## End\n");
    assert_eq!(doc.sections.len(), 4);
    let sections = &doc.sections;
    assert_ne!(sections[1].section_id, sections[2].section_id);
    assert_eq!(
        sections[1].parent_section_id.as_ref(),
        Some(&sections[0].section_id)
    );
    assert_eq!(sections[1].heading_path, ["Install", "Same"]);
    let second = doc
        .blocks
        .iter()
        .find(|block| block.retrieval_text == "second")
        .unwrap();
    assert_eq!(
        second.parent_section_id.as_ref(),
        Some(&sections[2].section_id)
    );
    assert_eq!(doc.blocks[0].parent_section_id, None);
}

#[test]
fn unicode_spans_index_the_unchanged_markdown() {
    let md = concat!(
        "---\ntitle: Café\nsource_url: https://example.test/doc\n---\n",
        "# Café 🦀\n\nDo **not** use 9.0.21; use `9.0.22` at 20 ms.\n"
    );
    let doc = document(md);
    for block in &doc.blocks {
        assert_eq!(block.revision_id, doc.revision_id);
        assert!(!block.source_spans.is_empty());
        for span in &block.source_spans {
            assert!(span.is_valid(md));
        }
    }
    let heading = doc
        .blocks
        .iter()
        .find(|block| block.block_type == BlockType::Heading)
        .unwrap();
    let span = heading.source_spans[0];
    assert_eq!(&md[span.start..span.end], "# Café 🦀\n");
    assert_eq!(heading.retrieval_text, "Café 🦀");
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.retrieval_text == "Do not use 9.0.21; use 9.0.22 at 20 ms.")
    );
    assert_eq!(doc.original_markdown_reference.byte_length, md.len());
    assert_eq!(doc.source_metadata.title.as_deref(), Some("Café"));
}

#[test]
fn nested_lists_quotes_links_and_images_retain_structure() {
    let md = concat!(
        "# Main\n\n1. **Do not** stop\n   - nested [guide](guide.md#part)\n",
        "   - ![plot](missing.png \"Chart\")\n\n> ### Quoted\n> stay here\n\noutside\n"
    );
    let doc = document(md);
    assert_eq!(
        doc.blocks
            .iter()
            .filter(|block| block.block_type == BlockType::List)
            .count(),
        2
    );
    let nested = doc
        .blocks
        .iter()
        .find(|block| block.block_type == BlockType::List && block.parent_block_id.is_some())
        .unwrap();
    assert!(doc.blocks.iter().any(|block| Some(&block.block_id)
        == nested.parent_block_id.as_ref()
        && block.block_type == BlockType::ListItem));
    assert!(
        doc.links
            .iter()
            .any(|link| link.destination == "guide.md#part" && !link.image)
    );
    assert!(
        doc.links
            .iter()
            .any(|link| link.destination == "missing.png" && link.image && link.label == "plot")
    );
    let outside = doc
        .blocks
        .iter()
        .find(|block| block.retrieval_text == "outside")
        .unwrap();
    assert_eq!(outside.heading_path, ["Main"]);
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.code == "asset_unchecked")
    );
}

#[test]
fn code_preserves_indentation_language_and_fence_style() {
    let doc = document(
        "```python extra\nif not ready:\n    sleep(20)\n```\n\n    x = 9.022\n    print(x)\n",
    );
    let code: Vec<_> = doc
        .blocks
        .iter()
        .filter(|block| block.block_type == BlockType::Code)
        .collect();
    assert_eq!(code.len(), 2);
    assert_eq!(code[0].retrieval_text, "if not ready:\n    sleep(20)\n");
    assert!(matches!(&code[0].structured_content.attributes,
        BlockAttributes::Code {
            style: CodeKind::Fenced,
            info: Some(info),
            language: Some(language),
        } if info == "python extra" && language == "python"));
    assert_eq!(code[1].retrieval_text, "x = 9.022\nprint(x)\n");
    assert!(matches!(
        code[1].structured_content.attributes,
        BlockAttributes::Code {
            style: CodeKind::Indented,
            ..
        }
    ));
}

#[test]
fn tables_preserve_headers_cells_alignment_and_values() {
    let doc = document("| Version | Wait |\n|:---|---:|\n| 9.0.22 | 20 ms |\n| no | 0 |\n");
    let table = doc
        .blocks
        .iter()
        .find(|block| block.block_type == BlockType::Table)
        .unwrap();
    assert!(matches!(&table.structured_content.attributes,
        BlockAttributes::Table { alignments } if alignments == &["left", "right"]));
    assert_eq!(table.retrieval_text, "Version\tWait\n9.0.22\t20 ms\nno\t0");
    assert_eq!(
        doc.blocks
            .iter()
            .filter(|block| block.block_type == BlockType::TableHead)
            .count(),
        1
    );
    assert_eq!(
        doc.blocks
            .iter()
            .filter(|block| block.block_type == BlockType::TableCell)
            .count(),
        6
    );
}

#[test]
fn footnotes_and_reference_definitions_remain_traceable() {
    let md = concat!(
        "See [manual][m] and note[^n].\n\n",
        "[m]: https://example.test/manual \"Guide\"\n\n[^n]: Do not restart.\n"
    );
    let doc = document(md);
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.block_type == BlockType::FootnoteDefinition)
    );
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.block_type == BlockType::ReferenceDefinition)
    );
    assert!(
        doc.links
            .iter()
            .any(|link| link.destination == "https://example.test/manual" && link.title == "Guide")
    );
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.retrieval_text.contains("note[^n]"))
    );
}

#[test]
fn empty_documents_fail_without_fabricated_provenance() {
    let doc = document(" \n\t\n");
    assert_eq!(doc.validation_status, ValidationStatus::Failed);
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.code == "empty_document" && warning.severity == Severity::Error)
    );
    assert_eq!(doc.source_reference, None);
    assert_eq!(doc.access_policy, None);
    assert_eq!(doc.source_metadata.extraction, None);
    let metadata_only = document("---\ntitle: Alone\n---\n");
    assert_eq!(metadata_only.validation_status, ValidationStatus::Failed);
}

#[test]
fn identical_text_from_different_sources_keeps_distinct_identity() {
    let mut first = CanonicalizeInput::new("Same text", "a.md");
    first.metadata.source_reference = Some("https://example.test/a".into());
    let mut second = first.clone();
    second.metadata.source_reference = Some("https://example.test/b".into());
    let from_a = canonicalize(first).unwrap();
    let from_b = canonicalize(second).unwrap();
    assert_eq!(from_a.content_hash, from_b.content_hash);
    assert_ne!(from_a.document_id, from_b.document_id);
    assert_ne!(from_a.revision_id, from_b.revision_id);
}

#[test]
fn repeated_processing_produces_identical_serialized_documents() {
    let md = "# Repeated\n\nText with café.\n\n# Repeated\n\nmore\n";
    let once = document(md);
    let again = document(md);
    assert_eq!(
        serde_json::to_vec(&once).unwrap(),
        serde_json::to_vec(&again).unwrap()
    );
    let changed = document("# Repeated\n\nChanged\n");
    assert_eq!(once.document_id, changed.document_id);
    assert_ne!(once.revision_id, changed.revision_id);
    let mut options = CanonicalizeInput::new(md, "fixtures/source.md");
    options.parser_options.tables = false;
    let alternative = canonicalize(options).unwrap();
    assert_eq!(once.revision_id, alternative.revision_id);
    assert_ne!(once.blocks[0].block_id, alternative.blocks[0].block_id);
}

#[test]
fn asset_inventory_is_explicit_and_missing_assets_warn() {
    let mut input =
        CanonicalizeInput::new("![missing](missing.png) ![present](plot.svg)\n", "doc.md");
    input
        .assets
        .insert("missing.png".into(), AssetStatus::Missing);
    input
        .assets
        .insert("plot.svg".into(), AssetStatus::Available);
    let doc = canonicalize(input).unwrap();
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.code == "missing_asset")
    );
    assert_ne!(doc.validation_status, ValidationStatus::Failed);
    assert!(
        doc.blocks
            .iter()
            .flat_map(|block| &block.asset_references)
            .any(|asset| asset.status == AssetStatus::Available)
    );
}

#[test]
fn supplied_extractor_structure_and_locations_are_retained() {
    let mut input = CanonicalizeInput::new("Café text\n", "doc.md");
    let supplied = ExtractorBlock {
        extractor_id: "from-extractor".into(),
        markdown_spans: vec![SourceSpan { start: 0, end: 10 }],
        original_locations: vec![OriginalLocation {
            source_reference: Some("manual.pdf".into()),
            page: Some(8),
            locator: Some(json!({"bbox":[1,2,3,4]})),
        }],
        structured_content: json!({"type":"text", "text":"Café text"}),
    };
    input.extractor_blocks.push(supplied.clone());
    let doc = canonicalize(input.clone()).unwrap();
    assert_eq!(doc.extractor_blocks, vec![supplied]);
    assert_eq!(doc.blocks[0].extractor_block_ids, ["from-extractor"]);
    input.extractor_blocks[0].markdown_spans[0].end = 4; // splits é
    let bad = canonicalize(input).unwrap();
    assert_eq!(bad.validation_status, ValidationStatus::Failed);
    assert!(
        bad.warnings
            .iter()
            .any(|warning| warning.code == "invalid_extractor_span")
    );
}

#[test]
fn malformed_frontmatter_and_html_are_not_silently_repaired() {
    let doc = document("---\ntitle: broken: yaml\n---\n\n<div>Do not delete 20 ms</div>\n");
    assert_eq!(doc.validation_status, ValidationStatus::Failed);
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.code == "invalid_frontmatter")
    );
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.code == "raw_html")
    );
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.retrieval_text.contains("Do not delete 20 ms"))
    );
}

#[test]
fn policy_conflicts_are_failures_not_permission_guesses() {
    let mut input = CanonicalizeInput::new("---\naccess_policy: private\n---\nBody", "doc.md");
    input.metadata.access_policy = Some(json!("public"));
    let doc = canonicalize(input).unwrap();
    assert_eq!(doc.validation_status, ValidationStatus::Failed);
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.code == "metadata_conflict")
    );
}
