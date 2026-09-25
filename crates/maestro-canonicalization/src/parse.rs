//! Offset-aware parser tree. Its nodes are temporary; the public result is an arena.
use crate::content::{BlockAttributes, CodeKind, InlineKind};
use crate::error::Error;
use crate::model::{ParserOptions, SourceSpan};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// What a parser node is: a block with its attributes, or inline content.
#[derive(Debug, Clone)]
pub(crate) enum Kind {
    /// A block with its attributes.
    Block(BlockAttributes),
    /// Inline content with its values.
    Inline(InlineKind),
}
/// A temporary parser node and the source span it came from.
#[derive(Debug, Clone)]
pub(crate) struct Node {
    /// Block or inline, with its values.
    pub kind: Kind,
    /// The original bytes the node spans.
    pub span: SourceSpan,
    /// Nested nodes, in source order.
    pub children: Vec<Node>,
}

/// The parser flags for a profile: YAML metadata always on, each extension as configured and smart
/// punctuation off.
pub(crate) fn options(config: &ParserOptions) -> Options {
    let mut options = Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    for (enabled, flag) in [
        (config.tables, Options::ENABLE_TABLES),
        (config.footnotes, Options::ENABLE_FOOTNOTES),
        (config.strikethrough, Options::ENABLE_STRIKETHROUGH),
        (config.tasklists, Options::ENABLE_TASKLISTS),
        (
            config.heading_attributes,
            Options::ENABLE_HEADING_ATTRIBUTES,
        ),
        (config.definition_lists, Options::ENABLE_DEFINITION_LIST),
        (config.math, Options::ENABLE_MATH),
        (config.gfm, Options::ENABLE_GFM),
    ] {
        options.set(flag, enabled);
    }
    // Smart punctuation is deliberately off: commands and identifiers must not change.
    options
}

/// Parse Markdown into a tree of nodes with source spans, reference definitions included and roots
/// in source order.
pub(crate) fn parse(markdown: &str, config: &ParserOptions) -> Result<Vec<Node>, Error> {
    let parser = Parser::new_ext(markdown, options(config));
    let definitions: Vec<_> = parser
        .reference_definitions()
        .iter()
        .map(|(label, def)| Node {
            kind: Kind::Block(BlockAttributes::ReferenceDefinition {
                label: label.to_string(),
                destination: def.dest.to_string(),
                title: def.title.as_deref().unwrap_or("").into(),
            }),
            span: SourceSpan {
                start: def.span.start,
                end: def.span.end,
            },
            children: Vec::new(),
        })
        .collect();
    let mut roots = Vec::new();
    let mut stack: Vec<(TagEnd, Node)> = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        let span = SourceSpan {
            start: range.start,
            end: range.end,
        };
        if !span.is_valid(markdown) {
            return Err(Error("parser emitted an invalid UTF-8 span".into()));
        }
        match event {
            Event::Start(tag) => {
                if stack.len() >= 128 {
                    return Err(Error("Markdown nesting exceeds safety limit 128".into()));
                }
                stack.push((
                    tag.to_end(),
                    Node {
                        kind: tag_kind(tag),
                        span,
                        children: Vec::new(),
                    },
                ));
            }
            Event::End(end) => {
                let Some((expected, mut node)) = stack.pop() else {
                    return Err(Error("parser emitted an unmatched closing event".into()));
                };
                if end != expected {
                    return Err(Error("parser emitted inconsistent nesting".into()));
                }
                node.span.end = node.span.end.max(span.end);
                append(node, &mut stack, &mut roots);
            }
            leaf => append(
                Node {
                    kind: leaf_kind(leaf)?,
                    span,
                    children: Vec::new(),
                },
                &mut stack,
                &mut roots,
            ),
        }
    }
    if !stack.is_empty() {
        return Err(Error("parser left unclosed containers".into()));
    }
    // Definitions are not emitted as events by pulldown-cmark. Reinsert at their
    // actual source position, including definitions nested inside containers.
    for definition in definitions {
        insert_definition(&mut roots, definition);
    }
    roots.sort_by_key(|n| n.span.start);
    Ok(roots)
}

/// Keep source the parser dropped as raw blocks, each with the reason it was not parsed.
pub(crate) fn preserve_gaps(
    nodes: &mut Vec<Node>,
    markdown: &str,
    mut gaps: Vec<(SourceSpan, String)>,
) {
    gaps.sort_by_key(|(span, _)| (span.start, span.end));
    gaps.dedup_by_key(|(span, _)| (span.start, span.end));
    for (span, reason) in gaps {
        if span.start >= span.end || !span.is_valid(markdown) {
            continue;
        }
        let raw = Node {
            kind: Kind::Block(BlockAttributes::Raw { reason }),
            span,
            children: vec![Node {
                kind: Kind::Inline(InlineKind::Text {
                    text: markdown[span.start..span.end].to_owned(),
                }),
                span,
                children: Vec::new(),
            }],
        };
        insert_definition(nodes, raw);
    }
}

/// Add a finished node to its open parent, or to the roots.
fn append(node: Node, stack: &mut [(TagEnd, Node)], roots: &mut Vec<Node>) {
    if let Some((_, parent)) = stack.last_mut() {
        parent.children.push(node);
    } else {
        roots.push(node);
    }
}

/// Place a node inside the innermost block that contains its span, else among the roots in source
/// order.
fn insert_definition(nodes: &mut Vec<Node>, definition: Node) {
    if let Some(parent) = nodes.iter_mut().find(|n| {
        matches!(n.kind, Kind::Block(_))
            && n.span.start <= definition.span.start
            && definition.span.end <= n.span.end
    }) {
        insert_definition(&mut parent.children, definition);
    } else {
        nodes.push(definition);
        nodes.sort_by_key(|n| n.span.start);
    }
}

/// The node kind a parser start tag opens.
fn tag_kind(tag: Tag<'_>) -> Kind {
    let block = match tag {
        Tag::Paragraph => BlockAttributes::Paragraph,
        Tag::Heading {
            level,
            id,
            classes,
            attrs,
        } => BlockAttributes::Heading {
            level: level as u8,
            explicit_id: id.map(|explicit| explicit.to_string()),
            classes: classes.iter().map(ToString::to_string).collect(),
            attributes: attrs
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.map(|text| text.to_string())))
                .collect(),
        },
        Tag::CodeBlock(CodeBlockKind::Indented) => BlockAttributes::Code {
            style: CodeKind::Indented,
            info: None,
            language: None,
        },
        Tag::CodeBlock(CodeBlockKind::Fenced(info)) => BlockAttributes::Code {
            style: CodeKind::Fenced,
            language: info.split_whitespace().next().map(str::to_owned),
            info: Some(info.to_string()),
        },
        Tag::BlockQuote(kind) => BlockAttributes::BlockQuote {
            alert: kind.map(|alert| format!("{alert:?}").to_lowercase()),
        },
        Tag::List(start) => BlockAttributes::List { start },
        Tag::Item => BlockAttributes::ListItem,
        Tag::Table(alignments) => BlockAttributes::Table {
            alignments: alignments
                .iter()
                .map(|alignment| format!("{alignment:?}").to_lowercase())
                .collect(),
        },
        Tag::TableHead => BlockAttributes::TableHead,
        Tag::TableRow => BlockAttributes::TableRow,
        Tag::TableCell => BlockAttributes::TableCell,
        Tag::FootnoteDefinition(label) => BlockAttributes::FootnoteDefinition {
            label: label.to_string(),
        },
        Tag::HtmlBlock => BlockAttributes::Html,
        Tag::MetadataBlock(_) => BlockAttributes::Metadata,
        Tag::DefinitionList => BlockAttributes::DefinitionList,
        Tag::DefinitionListTitle => BlockAttributes::DefinitionTerm,
        Tag::DefinitionListDefinition => BlockAttributes::DefinitionDescription,
        Tag::Emphasis => return Kind::Inline(InlineKind::Emphasis),
        Tag::Strong => return Kind::Inline(InlineKind::Strong),
        Tag::Strikethrough => return Kind::Inline(InlineKind::Strikethrough),
        Tag::Superscript => return Kind::Inline(InlineKind::Superscript),
        Tag::Subscript => return Kind::Inline(InlineKind::Subscript),
        Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => {
            return Kind::Inline(InlineKind::Link {
                destination: dest_url.to_string(),
                title: title.to_string(),
                reference: id.to_string(),
                link_type: format!("{link_type:?}"),
            });
        }
        Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        } => {
            return Kind::Inline(InlineKind::Image {
                destination: dest_url.to_string(),
                title: title.to_string(),
                reference: id.to_string(),
                link_type: format!("{link_type:?}"),
            });
        }
    };
    Kind::Block(block)
}

/// The node kind of a leaf event: text, code, math, HTML, a note reference, a break, a task marker
/// or a thematic break.
fn leaf_kind(event: Event<'_>) -> Result<Kind, Error> {
    let inline = match event {
        Event::Text(text) => InlineKind::Text {
            text: text.to_string(),
        },
        Event::Code(text) => InlineKind::Code {
            text: text.to_string(),
        },
        Event::InlineMath(text) => InlineKind::Math {
            text: text.to_string(),
            display: false,
        },
        Event::DisplayMath(text) => InlineKind::Math {
            text: text.to_string(),
            display: true,
        },
        Event::Html(raw) | Event::InlineHtml(raw) => InlineKind::Html {
            raw: raw.to_string(),
        },
        Event::FootnoteReference(label) => InlineKind::FootnoteReference {
            label: label.to_string(),
        },
        Event::SoftBreak => InlineKind::SoftBreak,
        Event::HardBreak => InlineKind::HardBreak,
        Event::TaskListMarker(checked) => InlineKind::TaskMarker { checked },
        Event::Rule => return Ok(Kind::Block(BlockAttributes::ThematicBreak)),
        Event::Start(_) | Event::End(_) => {
            return Err(Error("container received as a leaf".into()));
        }
    };
    Ok(Kind::Inline(inline))
}

/// A node's retrieval text: inline text as read, code and raw source as written, table cells
/// tab-separated and list items with their markers.
pub(crate) fn text(node: &Node) -> String {
    let children: Vec<_> = node.children.iter().map(text).collect();
    match &node.kind {
        Kind::Inline(kind) => inline_text(kind, &children.concat()),
        Kind::Block(
            BlockAttributes::Code { .. }
            | BlockAttributes::Html
            | BlockAttributes::Metadata
            | BlockAttributes::Raw { .. },
        ) => children.concat(),
        // Raw attribute fallbacks are evidence, not part of the resolved heading title.
        Kind::Block(BlockAttributes::Heading { .. }) => node
            .children
            .iter()
            .filter(|child| matches!(child.kind, Kind::Inline(_)))
            .map(text)
            .collect(),
        Kind::Block(BlockAttributes::TableHead | BlockAttributes::TableRow) => children.join("\t"),
        Kind::Block(BlockAttributes::Table { .. }) => children.join("\n"),
        Kind::Block(BlockAttributes::List { start }) => children
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let marker = match start {
                    Some(n) => format!("{}. ", u128::from(*n) + i as u128),
                    None => "- ".into(),
                };
                format!("{marker}{}", item.replace('\n', "\n  "))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Kind::Block(_) => {
            let mut output = String::new();
            for (child, rendered) in node.children.iter().zip(children) {
                if matches!(child.kind, Kind::Block(_))
                    && !output.is_empty()
                    && !output.ends_with('\n')
                {
                    output.push('\n');
                }
                output.push_str(&rendered);
                if matches!(child.kind, Kind::Block(_)) && !output.ends_with('\n') {
                    output.push('\n');
                }
            }
            output.trim_end_matches('\n').to_owned()
        }
    }
}

/// The text an inline node reads as, given its children's text; deletion and script markers are
/// kept.
pub(crate) fn inline_text(kind: &InlineKind, children: &str) -> String {
    match kind {
        InlineKind::Text { text } | InlineKind::Code { text } | InlineKind::Math { text, .. } => {
            text.clone()
        }
        InlineKind::Html { raw } => raw.clone(),
        InlineKind::FootnoteReference { label } => format!("[^{label}]"),
        InlineKind::SoftBreak | InlineKind::HardBreak => "\n".into(),
        InlineKind::TaskMarker { checked } => if *checked { "[x] " } else { "[ ] " }.into(),
        InlineKind::Image { destination, .. } => format!("{children} ({destination})"),
        // Deletion and script position carry meaning; don't silently erase them.
        InlineKind::Strikethrough => format!("~~{children}~~"),
        InlineKind::Superscript => format!("^({children})"),
        InlineKind::Subscript => format!("_({children})"),
        InlineKind::Emphasis | InlineKind::Strong | InlineKind::Link { .. } => children.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockType, CanonicalizeInput, canonicalize};

    #[test]
    fn only_nonempty_valid_gaps_become_raw_blocks() {
        let mut nodes = Vec::new();
        for (start, end) in [(1, 1), (2, 9), (0, 2)] {
            preserve_gaps(
                &mut nodes,
                "abc",
                vec![(SourceSpan { start, end }, "gap".into())],
            );
        }
        let spans: Vec<_> = nodes.iter().map(|n| (n.span.start, n.span.end)).collect();
        assert_eq!(spans, [(0, 2)]);
    }

    #[test]
    fn an_empty_block_adds_no_blank_line_to_its_container_text() {
        let markdown = "> A\n>\n> ***\n>\n> B\n";
        let doc = canonicalize(CanonicalizeInput::new(markdown, "quote.md")).unwrap();
        let quote = doc
            .blocks
            .iter()
            .find(|block| block.block_type == BlockType::BlockQuote)
            .unwrap();
        assert_eq!(quote.retrieval_text, "A\nB");
    }
}
