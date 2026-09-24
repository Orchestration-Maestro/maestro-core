//! A deterministic, unique byte partition; nested block spans remain independently valid.
use crate::{BlockType, CanonicalDocument, ContentNode, Inline, InlineKind, SourceSpan};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};

/// What accounts for an original source-syntax range, not a normalized-text range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRole {
    /// Syntax contributing to a parsed textual value; may include escapes or inline delimiters.
    ParsedContent,
    /// Container delimiters, attributes, whitespace and other parser-recognized syntax.
    StructuralSyntax,
    /// Metadata, link destinations/titles, or reference definitions retained in typed fields.
    MetadataOrReference,
    /// Exact raw/fallback syntax, explicitly retained without interpreting its semantics.
    Unsupported,
    /// A source contribution lacking a representation; blocks document acceptance.
    Unaccounted,
}

/// One nonoverlapping segment in the exhaustive original-byte accounting ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAccounting {
    /// Inclusive start/exclusive end in the immutable original Markdown.
    pub source_span: SourceSpan,
    /// How this syntax is represented; never a statement about extraction accuracy.
    pub role: SourceRole,
    /// Innermost owning block, or null for inter-block whitespace/unaccounted source.
    pub block_id: Option<String>,
}

/// One claim on original bytes: its span, role, owning block and the rank that decides which claim
/// wins where claims overlap.
struct Contribution<'a> {
    /// The original bytes claimed.
    span: SourceSpan,
    /// How the claimed bytes are represented.
    role: SourceRole,
    /// The block the claimed bytes belong to.
    owner: &'a str,
    /// Precedence where claims overlap: the highest rank wins, then the narrowest span.
    rank: u8,
}

/// The source ledger: every original byte once, in order, with the role and innermost owner of the
/// claim that wins it; unclaimed non-whitespace is unaccounted.
pub(crate) fn account(doc: &CanonicalDocument, markdown: &str) -> Vec<SourceAccounting> {
    let mut contributions = Vec::new();
    for block in &doc.blocks {
        let (role, rank) = match block.block_type {
            BlockType::Metadata | BlockType::ReferenceDefinition => {
                (SourceRole::MetadataOrReference, 4)
            }
            BlockType::Raw | BlockType::Html => (SourceRole::Unsupported, 5),
            _ => (SourceRole::StructuralSyntax, 0),
        };
        for &span in &block.source_spans {
            contributions.push(Contribution {
                span,
                role,
                owner: &block.block_id,
                rank,
            });
        }
        for child in &block.structured_content.children {
            if let ContentNode::Inline { inline } = child {
                inline_contributions(inline, &block.block_id, &mut contributions);
            }
        }
    }
    // Sweep interval endpoints rather than comparing every byte to every block.
    let mut endpoints: BTreeMap<usize, Vec<(usize, bool)>> = BTreeMap::new();
    endpoints.entry(0).or_default();
    endpoints.entry(markdown.len()).or_default();
    for (id, c) in contributions.iter().enumerate() {
        if c.span.start < c.span.end && c.span.is_valid(markdown) {
            endpoints.entry(c.span.start).or_default().push((id, true));
            endpoints.entry(c.span.end).or_default().push((id, false));
        }
    }
    let mut active = BTreeSet::new();
    let mut result: Vec<SourceAccounting> = Vec::new();
    let mut previous = 0;
    for (position, events) in endpoints {
        if previous < position {
            let (role, owner) = match active.last().copied() {
                Some((_, _, id)) => {
                    let c: &Contribution<'_> = &contributions[id];
                    (c.role, Some(c.owner.to_owned()))
                }
                None if markdown[previous..position].trim().is_empty() => {
                    (SourceRole::StructuralSyntax, None)
                }
                None => (SourceRole::Unaccounted, None),
            };
            if let Some(last) = result
                .last_mut()
                .filter(|a| a.role == role && a.block_id == owner)
            {
                last.source_span.end = position;
            } else {
                result.push(SourceAccounting {
                    source_span: SourceSpan {
                        start: previous,
                        end: position,
                    },
                    role,
                    block_id: owner,
                });
            }
        }
        for (id, start) in events {
            let c = &contributions[id];
            let key = (c.rank, Reverse(c.span.end - c.span.start), id);
            if start {
                active.insert(key);
            } else {
                active.remove(&key);
            }
        }
        previous = position;
    }
    result
}

/// The claims of an inline node and its descendants: links and images as references, HTML as
/// unsupported, leaf text as parsed content.
fn inline_contributions<'a>(inline: &Inline, owner: &'a str, result: &mut Vec<Contribution<'a>>) {
    let classified = match inline.content {
        InlineKind::Link { .. } | InlineKind::Image { .. } => {
            Some((SourceRole::MetadataOrReference, 1))
        }
        InlineKind::Html { .. } => Some((SourceRole::Unsupported, 3)),
        _ if inline.children.is_empty() => Some((SourceRole::ParsedContent, 2)),
        _ => None,
    };
    if let Some((role, rank)) = classified {
        result.push(Contribution {
            span: inline.source_span,
            role,
            owner,
            rank,
        });
    }
    for child in &inline.children {
        inline_contributions(child, owner, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalizeInput, canonicalize};

    /// Each ledger segment as the text it covers and its role.
    fn segments<'a>(ledger: &[SourceAccounting], markdown: &'a str) -> Vec<(&'a str, SourceRole)> {
        ledger
            .iter()
            .map(|part| {
                (
                    &markdown[part.source_span.start..part.source_span.end],
                    part.role,
                )
            })
            .collect()
    }

    /// A document whose paragraph block is removed, leaving its bytes unclaimed.
    fn without_paragraph(markdown: &str) -> CanonicalDocument {
        let mut doc = canonicalize(CanonicalizeInput::new(markdown, "ledger.md")).unwrap();
        doc.blocks.retain(|b| b.block_type == BlockType::Heading);
        doc
    }

    #[test]
    fn inline_html_is_accounted_as_unsupported() {
        let markdown = "a <b>bold</b> c\n";
        let doc = canonicalize(CanonicalizeInput::new(markdown, "html.md")).unwrap();
        let ledger = segments(&doc.source_accounting, markdown);
        assert!(
            ledger.contains(&("<b>", SourceRole::Unsupported)),
            "{ledger:?}"
        );
    }

    #[test]
    fn unclaimed_text_is_unaccounted_and_never_counted_as_syntax() {
        let markdown = "# Title\n\nBody\n";
        let ledger = account(&without_paragraph(markdown), markdown);
        let ledger = segments(&ledger, markdown);
        assert!(
            ledger
                .iter()
                .any(|(text, role)| text.contains("Body") && *role == SourceRole::Unaccounted),
            "{ledger:?}"
        );
    }

    #[test]
    fn empty_and_invalid_spans_claim_no_bytes() {
        let markdown = "# Title\n\nBody\n";
        let doc = without_paragraph(markdown);
        let expected = account(&doc, markdown);
        let body = markdown.find("Body").unwrap();
        for span in [
            SourceSpan {
                start: body,
                end: body,
            },
            SourceSpan {
                start: body,
                end: markdown.len() + 4,
            },
        ] {
            let mut changed = doc.clone();
            changed.blocks[0].source_spans.push(span);
            assert_eq!(account(&changed, markdown), expected, "{span:?}");
        }
    }
}
