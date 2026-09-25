//! Generated Markdown dialects keep their spans, meaning and round trips, deterministically.

#![cfg(test)]
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, ContentNode, Inline, Severity, SourceRole,
    ValidationStatus, canonicalize, validate_document,
};

fn check_inline(inline: &Inline, source: &str) {
    let span = inline.source_span;
    assert!(span.start <= span.end && span.end <= source.len());
    assert!(source.get(span.start..span.end).is_some());
    for child in &inline.children {
        assert!(span.start <= child.source_span.start && child.source_span.end <= span.end);
        check_inline(child, source);
    }
}

#[test]
fn generated_unicode_dialects_preserve_spans_semantics_and_roundtrips() {
    let fragments = [
        "Café",
        "e\u{301}",
        "🦀",
        "日本語",
        "العربية",
        "\\*literal\\*",
        "`repoName`",
        "**not**",
        "a &amp; b",
        "a\tb",
        "line  \nnext",
        "~9.0.22~",
    ];
    // 12 fragments × 10 wrappers × 2 line endings × 2 extension profiles = 480 cases.
    for (fragment_id, fragment) in fragments.iter().enumerate() {
        let marker = format!("not V9.0.22 RepoName{fragment_id} 20 ms");
        let body = format!("{marker} {fragment}");
        let wrappers = [
            format!("{body}\n"),
            format!("# Repeat\n### Repeat\n\n{body}\n"),
            format!("> {}\n", body.replace('\n', "\n> ")),
            format!("- {}\n", body.replace('\n', "\n  ")),
            format!("```rust info=9.0.22\n{body}\n```\n"),
            format!("    {}\n", body.replace('\n', "\n    ")),
            format!("| Meaning |\n|---|\n|{}|\n", body.replace('\n', " ")),
            format!("[^note]: {}\n\nSee[^note].\n", body.replace('\n', "\n    ")),
            format!("<div>{body}</div>\n"),
            format!("# H {{#first #last}}\n\n{body}\n"),
        ];
        for (wrapper_id, wrapper) in wrappers.iter().enumerate() {
            let wrapped = format!("fragment={fragment_id} wrapper={wrapper_id}");
            check_every_line_ending_and_profile(&wrapped, wrapper, &marker);
        }
    }
}

/// Every property of one wrapped fragment, with LF and CRLF line endings,
/// without and with the parser's extensions.
fn check_every_line_ending_and_profile(wrapped: &str, wrapper: &str, marker: &str) {
    for crlf in [false, true] {
        let source = if crlf {
            wrapper.replace('\n', "\r\n")
        } else {
            wrapper.to_owned()
        };
        for extensions in [false, true] {
            let case = format!("{wrapped} crlf={crlf} extensions={extensions}");
            let doc = check_deterministic_valid_round_trip(&case, &source, extensions, marker);
            check_ledger_covers_every_byte_once(&case, &doc, &source);
            check_meaning_is_never_markup(&case, &doc, &source, marker);
            check_blocks_nest_inside_their_parents(&case, &doc, &source);
        }
    }
}

/// The source canonicalizes the same way twice, is not failed, survives a
/// JSON round trip and replay validation, grants no access and keeps its
/// marker searchable.
fn check_deterministic_valid_round_trip(
    case: &str,
    source: &str,
    extensions: bool,
    marker: &str,
) -> CanonicalDocument {
    let mut input = CanonicalizeInput::new(source, "generated.md");
    input.parser_options.tables = extensions;
    input.parser_options.footnotes = extensions;
    input.parser_options.heading_attributes = extensions;
    let doc = canonicalize(input.clone()).expect(case);
    assert_ne!(
        doc.validation_status,
        ValidationStatus::Failed,
        "{case}: {:?}",
        doc.warnings
    );
    assert_eq!(doc, canonicalize(input).expect(case), "{case}");
    let json = serde_json::to_vec(&doc).unwrap();
    let decoded: CanonicalDocument = serde_json::from_slice(&json).unwrap();
    assert_eq!(doc, decoded, "{case}");
    assert!(
        !validate_document(&decoded, source)
            .iter()
            .any(|finding| finding.severity == Severity::Error),
        "{case}"
    );
    assert!(doc.access_policy.is_none(), "{case}");
    assert!(
        doc.blocks
            .iter()
            .any(|block| block.retrieval_text.contains(marker)),
        "{case}"
    );
    doc
}

/// The source ledger accounts every byte once, in order, with a known role.
fn check_ledger_covers_every_byte_once(case: &str, doc: &CanonicalDocument, source: &str) {
    let mut cursor = 0;
    for part in &doc.source_accounting {
        assert_eq!(cursor, part.source_span.start, "{case}");
        assert!(part.source_span.end > cursor, "{case}");
        assert!(source.get(cursor..part.source_span.end).is_some(), "{case}");
        assert_ne!(part.role, SourceRole::Unaccounted, "{case}");
        cursor = part.source_span.end;
    }
    assert_eq!(cursor, source.len(), "{case}");
}

/// The marker's bytes are never labelled as structural markup.
fn check_meaning_is_never_markup(case: &str, doc: &CanonicalDocument, source: &str, marker: &str) {
    for (start, value) in source.match_indices(marker) {
        assert_eq!(&source[start..start + value.len()], marker, "{case}");
        for part in &doc.source_accounting {
            if part.source_span.start < start + value.len() && start < part.source_span.end {
                assert_ne!(
                    part.role,
                    SourceRole::StructuralSyntax,
                    "{case}: meaning mislabeled as markup"
                );
            }
        }
    }
}

/// Every block belongs to the revision, and its spans and inline content sit
/// inside the source and inside its parent's spans.
fn check_blocks_nest_inside_their_parents(case: &str, doc: &CanonicalDocument, source: &str) {
    for block in &doc.blocks {
        assert_eq!(block.revision_id, doc.revision_id, "{case}");
        for span in &block.source_spans {
            assert!(source.get(span.start..span.end).is_some(), "{case}");
            if let Some(parent) = &block.parent_block_id {
                let parent = doc
                    .blocks
                    .iter()
                    .find(|candidate| &candidate.block_id == parent)
                    .expect(case);
                assert!(
                    parent
                        .source_spans
                        .iter()
                        .any(|outer| outer.start <= span.start && span.end <= outer.end),
                    "{case}"
                );
            }
        }
        for child in &block.structured_content.children {
            if let ContentNode::Inline { inline } = child {
                check_inline(inline, source);
            }
        }
    }
}
