//! Tests of source mapping: order, Unicode, entities, envelopes and accounting.
use super::{map_accounting, map_document, mapped_slice};
use crate::{
    CanonicalDocument, canonicalize,
    chunks::{
        InlineEnvelope, MappedDocument, MappingRun, OriginMode, SourceDisposition, SourceOrigin,
        TextRange, UnitField,
    },
    content::{BlockType, ContentNode},
    model::{CanonicalizeInput, SourceSpan},
};

#[test]
fn decoded_entities_keep_original_syntax_origins() {
    let markdown = "A &amp; B\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "mapped-a")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    assert_eq!(
        mapped
            .units
            .iter()
            .filter(|unit| unit.primary)
            .map(|unit| unit.text.as_str())
            .collect::<String>(),
        "A & B"
    );
    assert!(
        mapped
            .units
            .iter()
            .flat_map(|unit| &unit.mappings)
            .any(|run| {
                run.mode == OriginMode::CanonicalTransformation
                    && run
                        .origins
                        .iter()
                        .any(|origin| &markdown[origin.span.start..origin.span.end] == "&amp;")
            })
    );
}

#[test]
fn mapped_units_preserve_order_unicode_structures_and_source_bytes() {
    for (name, markdown, expected) in [
        // The emoji is a woman, a skin tone, a zero-width joiner and a laptop.
        (
            "unicode",
            "café cafe\u{301} \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}\r\n",
            "café cafe\u{301} \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}",
        ),
        // The pinned parser retains NUL literally; Phase B must not repair it.
        ("nul", "left\0right\n", "left\0right"),
        ("line endings", "first\r\nsecond\n", "first\nsecond"),
        ("escapes", "Use \\*literal\\*.\n", "Use *literal*."),
        ("headings", "# First\n\n# Second\n", "FirstSecond"),
        ("thematic", "---\n", ""),
        (
            "inline",
            "~~do **not** delete~~ and `keep  spaces`\n",
            "~~do not delete~~ and keep  spaces",
        ),
        (
            "list",
            "- before\n  - child\n\n  after\n",
            "beforechildafter",
        ),
        ("metadata", "---\ntitle: Only metadata\n---\n", ""),
        (
            "table",
            "| Name | Value |\n|---|---|\n| alpha | 9.0.22 |\n",
            "NameValuealpha9.0.22",
        ),
        (
            "code",
            "```rust\n  let value = 1;\n```\n",
            "  let value = 1;\n",
        ),
        ("html", "<div>raw</div>\n", "<div>raw</div>\n"),
        (
            "footnote",
            "See [^note].\n\n[^note]: Keep this.\n",
            "See [^note].Keep this.",
        ),
        ("definition", "Term\n: Description\n", "TermDescription"),
    ] {
        let doc = canonicalize(CanonicalizeInput::new(markdown, name)).unwrap();
        let before = serde_json::to_vec(&doc).unwrap();
        let mapped = map_document(&doc, markdown).unwrap();
        check_primary_text_ledger_and_slices(name, markdown, expected, &doc, &mapped);
        if name == "metadata" {
            check_metadata_only_accounting(markdown, &doc, &mapped);
        }
        if name == "code" {
            check_code_info_is_context(&mapped);
        }
        if name == "list" {
            check_both_list_markers_are_units(&mapped);
        }
        assert_eq!(before, serde_json::to_vec(&doc).unwrap());
    }
}

/// Primary text in source order, one ledger entry per accounted source part,
/// eligible parts mapped to units, and every unit's full slice equal to its
/// mappings.
fn check_primary_text_ledger_and_slices(
    name: &str,
    markdown: &str,
    expected: &str,
    doc: &CanonicalDocument,
    mapped: &MappedDocument,
) {
    assert_eq!(
        mapped
            .units
            .iter()
            .filter(|unit| unit.primary)
            .map(|unit| unit.text.as_str())
            .collect::<String>(),
        expected,
        "{name}"
    );
    assert_eq!(mapped.accounting.len(), doc.source_accounting.len());
    for (index, entry) in mapped.accounting.iter().enumerate() {
        assert_eq!(entry.accounting_index, index);
        if entry.disposition == SourceDisposition::Eligible {
            assert!(!entry.unit_indices.is_empty());
        }
    }
    for unit in &mapped.units {
        let mappings = mapped_slice(
            unit,
            TextRange {
                start: 0,
                end: unit.text.len(),
            },
            markdown,
        )
        .unwrap();
        assert_eq!(mappings, unit.mappings);
    }
}

/// A metadata-only document accounts its front matter as metadata and
/// everything else as whitespace structure.
fn check_metadata_only_accounting(
    markdown: &str,
    doc: &CanonicalDocument,
    mapped: &MappedDocument,
) {
    assert!(
        mapped
            .accounting
            .iter()
            .any(|entry| entry.disposition == SourceDisposition::Metadata)
    );
    for entry in &mapped.accounting {
        if entry.disposition != SourceDisposition::Metadata {
            assert_eq!(entry.disposition, SourceDisposition::Structural);
            let span = doc.source_accounting[entry.accounting_index].source_span;
            assert!(markdown[span.start..span.end].trim().is_empty());
        }
    }
}

/// A fenced code block's info string is a non-primary code-info unit.
fn check_code_info_is_context(mapped: &MappedDocument) {
    assert!(
        mapped
            .units
            .iter()
            .any(|unit| !unit.primary && unit.field == UnitField::CodeInfo && unit.text == "rust")
    );
}

/// The parent's and the child's list markers are each their own unit.
fn check_both_list_markers_are_units(mapped: &MappedDocument) {
    assert_eq!(
        mapped
            .units
            .iter()
            .filter(|unit| unit.field == UnitField::ListMarker)
            .count(),
        2
    );
}

#[test]
fn exact_slices_narrow_but_transformed_slices_keep_original_syntax() {
    let markdown = "café &amp; done\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "slices")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let plain = mapped
        .units
        .iter()
        .find(|unit| unit.text == "café ")
        .unwrap();
    let narrowed = mapped_slice(plain, TextRange { start: 3, end: 5 }, markdown).unwrap();
    assert_eq!(narrowed[0].range, TextRange { start: 0, end: 2 });
    assert_eq!(narrowed[0].origins[0].span, SourceSpan { start: 3, end: 5 });
    assert!(mapped_slice(plain, TextRange { start: 3, end: 4 }, markdown).is_err());
    assert!(mapped_slice(plain, TextRange { start: 5, end: 3 }, markdown).is_err());
    let entity = mapped.units.iter().find(|unit| unit.text == "&").unwrap();
    let transformed = mapped_slice(entity, TextRange { start: 0, end: 1 }, markdown).unwrap();
    assert_eq!(
        transformed[0].origins[0].span,
        SourceSpan { start: 6, end: 11 }
    );
    let mut corrupted = plain.clone();
    corrupted.mappings[0].origins[0].span.end = 3;
    assert!(mapped_slice(&corrupted, TextRange { start: 0, end: 1 }, markdown).is_err());
}

#[test]
fn reference_images_keep_the_resolved_definition_not_a_same_url_neighbor() {
    let markdown = "![caption][  PIC  ]\n\n[wrong]: image.svg\n[pic]: image.svg\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "reference")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let image = mapped.units.iter().find(|unit| unit.primary).unwrap();
    assert_eq!(image.text, "caption (image.svg)");
    let origins: Vec<_> = image
        .mappings
        .iter()
        .flat_map(|run| &run.origins)
        .map(|origin| &markdown[origin.span.start..origin.span.end])
        .collect();
    assert!(origins.iter().any(|text| text.starts_with("![caption]")));
    assert!(origins.iter().any(|text| text.starts_with("[pic]:")));
    assert!(!origins.iter().any(|text| text.starts_with("[wrong]:")));
    assert_eq!(
        mapped
            .accounting
            .iter()
            .filter(|entry| entry.disposition == SourceDisposition::ReferenceDefinition)
            .count(),
        2
    );
}

#[test]
fn deletion_wrappers_keep_semantic_envelopes_and_source_origins() {
    let markdown = "~~do **not** delete~~\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "deletion")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let unit = mapped.units.iter().find(|unit| unit.primary).unwrap();
    assert_eq!(
        unit.envelopes,
        [InlineEnvelope {
            opening: TextRange { start: 0, end: 2 },
            closing: TextRange { start: 15, end: 17 }
        }]
    );
    for run in [&unit.mappings[0], unit.mappings.last().unwrap()] {
        assert_eq!(run.mode, OriginMode::CanonicalTransformation);
        assert!(!run.origins.is_empty());
    }
}

#[test]
fn missing_original_accounting_refuses_mapping() {
    let markdown = "visible\n";
    let mut doc = canonicalize(CanonicalizeInput::new(markdown, "accounting")).unwrap();
    doc.source_accounting[0].role = crate::SourceRole::Unaccounted;
    assert!(map_document(&doc, markdown).is_err());
    doc.source_accounting.clear();
    assert!(map_document(&doc, markdown).is_err());
}

#[test]
fn ledger_parts_carry_only_the_units_that_overlap_them() {
    let markdown = "# Title\n\nBody\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "touching")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let units: Vec<_> = mapped
        .accounting
        .iter()
        .map(|entry| entry.unit_indices.clone())
        .collect();
    // `# ` ends where the title's origin starts, and the newline starts where it ends.
    assert_eq!(units, [vec![], vec![0], vec![], vec![], vec![1], vec![]]);
}

#[test]
fn a_shifted_or_empty_ledger_part_refuses_mapping() {
    let markdown = "Body text\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "ledger")).unwrap();
    let units = map_document(&doc, markdown).unwrap().units;
    map_accounting(&doc, markdown, &units).unwrap();
    let mut shifted = doc.clone();
    shifted.source_accounting[0].source_span.start = 1;
    assert!(map_accounting(&shifted, markdown, &units).is_err());
    let mut empty = doc;
    let mut part = empty.source_accounting[0].clone();
    part.source_span.end = part.source_span.start;
    empty.source_accounting.insert(0, part);
    assert!(map_accounting(&empty, markdown, &units).is_err());
}

#[test]
fn front_matter_bytes_are_metadata_whatever_their_role() {
    let markdown = "---\ntitle: T\n---\nBody\n";
    let mut doc = canonicalize(CanonicalizeInput::new(markdown, "front")).unwrap();
    let units = map_document(&doc, markdown).unwrap().units;
    let front = doc
        .source_accounting
        .iter()
        .position(|entry| entry.role == crate::SourceRole::MetadataOrReference)
        .unwrap();
    doc.source_accounting[front].role = crate::SourceRole::ParsedContent;
    let accounting = map_accounting(&doc, markdown, &units).unwrap();
    assert_eq!(accounting[front].disposition, SourceDisposition::Metadata);
}

#[test]
fn each_reference_image_names_its_own_definition_block() {
    let markdown = "![one][a] ![two][b]\n\n[a]: a.svg\n\n[b]: b.svg\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "definitions")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let definitions: Vec<_> = mapped
        .units
        .iter()
        .flat_map(|unit| &unit.mappings)
        .flat_map(|run| &run.origins)
        .filter(|origin| markdown[origin.span.start..origin.span.end].starts_with('['))
        .collect();
    assert_eq!(definitions.len(), 2);
    for origin in definitions {
        let owner = doc
            .blocks
            .iter()
            .find(|block| block.block_id == origin.block_id)
            .unwrap();
        assert_eq!(owner.block_type, BlockType::ReferenceDefinition);
        assert!(owner.source_spans.contains(&origin.span));
    }
}

#[test]
fn text_with_an_invalid_origin_refuses_mapping() {
    let markdown = "Body\n";
    let mut doc = canonicalize(CanonicalizeInput::new(markdown, "origin")).unwrap();
    let ContentNode::Inline { inline } = &mut doc.blocks[0].structured_content.children[0] else {
        panic!("a paragraph starts with inline text");
    };
    inline.source_span.end = markdown.len() + 5;
    assert!(map_document(&doc, markdown).is_err());
}

#[test]
fn slices_refuse_gaps_empty_runs_and_origins_that_contradict_their_mode() {
    use OriginMode::{CanonicalTransformation as Transformed, ExactCopy as Exact, Formatting};
    let markdown = "abcde\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "runs")).unwrap();
    let base = map_document(&doc, markdown).unwrap().units.remove(0);
    assert_eq!(base.text, "abcde");
    let origin = |start, end| SourceOrigin {
        block_id: base.block_id.clone(),
        span: SourceSpan { start, end },
    };
    let run = |start, end, mode, origins| MappingRun {
        range: TextRange { start, end },
        mode,
        origins,
    };
    let slices = |mappings: Vec<MappingRun>| {
        let mut unit = base.clone();
        unit.mappings = mappings;
        mapped_slice(&unit, TextRange { start: 0, end: 5 }, markdown).is_ok()
    };
    assert!(slices(vec![run(0, 5, Exact, vec![origin(0, 5)])]));
    assert!(slices(vec![run(0, 5, Formatting, vec![])]));
    let gap = vec![
        run(0, 2, Exact, vec![origin(0, 2)]),
        run(3, 5, Exact, vec![origin(3, 5)]),
    ];
    assert!(!slices(gap));
    let empty = vec![
        run(0, 2, Exact, vec![origin(0, 2)]),
        run(2, 2, Formatting, vec![]),
        run(2, 5, Exact, vec![origin(2, 5)]),
    ];
    assert!(!slices(empty));
    assert!(!slices(vec![run(0, 5, Transformed, vec![])]));
    assert!(!slices(vec![run(0, 5, Formatting, vec![origin(0, 5)])]));
}
