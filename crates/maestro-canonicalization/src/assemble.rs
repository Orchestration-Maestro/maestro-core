//! Build natural blocks and lexical heading context from the offset-aware tree.
use crate::parse::{Kind, Node, inline_text, text};
use crate::{
    AssetReference, AssetStatus, Block, BlockAttributes, CanonicalDocument, ContentNode, Error,
    Inline, InlineKind, Link, Section, StructuredContent, digest,
};

/// Build the document's blocks, sections, links and asset references from the parsed tree.
pub(crate) fn assemble(nodes: &[Node], doc: &mut CanonicalDocument) -> Result<(), Error> {
    let config = serde_json::to_vec(&(&doc.parser_version, &doc.parser_options))
        .map_err(|e| Error(e.to_string()))?;
    let mut builder = Builder {
        doc,
        config: digest(&config),
    };
    builder.siblings(nodes, None, &mut Vec::new())?;
    Ok(())
}

/// The document under construction and the parser configuration its block identifiers include.
struct Builder<'a> {
    /// The document receiving blocks, sections and links.
    doc: &'a mut CanonicalDocument,
    /// Digest of the parser version and options, part of every block identifier.
    config: String,
}
impl Builder<'_> {
    /// Build a run of sibling nodes under a parent block, inside the current heading sections.
    fn siblings(
        &mut self,
        nodes: &[Node],
        parent: Option<&str>,
        context: &mut Vec<Section>,
    ) -> Result<Vec<ContentNode>, Error> {
        nodes
            .iter()
            .map(|node| match &node.kind {
                Kind::Block(attributes) => self.block(node, attributes, parent, context),
                Kind::Inline(_) => Ok(ContentNode::Inline {
                    inline: inline(node)?,
                }),
            })
            .collect()
    }

    /// Build one block: its identifier, text, section, heading path, children and asset references.
    fn block(
        &mut self,
        node: &Node,
        attributes: &BlockAttributes,
        parent: Option<&str>,
        context: &mut Vec<Section>,
    ) -> Result<ContentNode, Error> {
        let index = self.doc.blocks.len();
        let id = format!(
            "block-{}",
            digest(
                format!(
                    "{}\0{}\0{}\0{}:{}",
                    self.doc.revision_id, self.config, index, node.span.start, node.span.end
                )
                .as_bytes()
            )
        );
        let rendered = text(node);
        let mut parent_section = context.last().map(|s| s.section_id.clone());
        if let BlockAttributes::Heading { level, .. } = attributes {
            while context.last().is_some_and(|s| s.level >= *level) {
                context.pop();
            }
            parent_section = context.last().map(|s| s.section_id.clone());
            let mut path: Vec<_> = context.iter().map(|s| s.title.clone()).collect();
            path.push(rendered.clone());
            let section = Section {
                section_id: id.clone(),
                parent_section_id: parent_section.clone(),
                level: *level,
                title: rendered.clone(),
                heading_path: path,
            };
            context.push(section.clone());
            self.doc.sections.push(section);
        }
        let extractor_block_ids = self
            .doc
            .extractor_blocks
            .iter()
            .filter(|e| {
                e.markdown_spans
                    .iter()
                    .any(|s| s.start < node.span.end && node.span.start < s.end)
            })
            .map(|e| e.extractor_id.clone())
            .collect();
        self.doc.blocks.push(Block {
            block_id: id.clone(),
            revision_id: self.doc.revision_id.clone(),
            block_type: attributes.block_type(),
            parent_block_id: parent.map(str::to_owned),
            parent_section_id: parent_section,
            heading_path: context.iter().map(|s| s.title.clone()).collect(),
            source_spans: vec![node.span],
            retrieval_text: rendered,
            structured_content: StructuredContent {
                attributes: attributes.clone(),
                children: Vec::new(),
            },
            asset_references: Vec::new(),
            extractor_block_ids,
        });
        // Headings inside a quote/list/footnote are local to that container.
        let children = self.siblings(&node.children, Some(&id), &mut context.clone())?;
        let mut assets = Vec::new();
        for child in &children {
            if let ContentNode::Inline { inline } = child {
                self.references(inline, &id, &mut assets);
            }
        }
        self.doc.blocks[index].structured_content.children = children;
        self.doc.blocks[index].asset_references = assets;
        Ok(ContentNode::Block { block_id: id })
    }

    /// Record the links and images of an inline node and its descendants, with each destination's
    /// asset status.
    fn references(&mut self, inline: &Inline, block_id: &str, assets: &mut Vec<AssetReference>) {
        match &inline.content {
            InlineKind::Link {
                destination, title, ..
            }
            | InlineKind::Image {
                destination, title, ..
            } => {
                self.doc.links.push(Link {
                    block_id: block_id.into(),
                    destination: destination.clone(),
                    title: title.clone(),
                    label: inline.children.iter().map(render_inline).collect(),
                    image: matches!(inline.content, InlineKind::Image { .. }),
                    source_span: inline.source_span,
                });
                assets.push(AssetReference {
                    destination: destination.clone(),
                    source_span: inline.source_span,
                    status: destination_status(
                        destination,
                        self.doc.asset_inventory.get(destination),
                    ),
                });
            }
            _ => {}
        }
        for child in &inline.children {
            self.references(child, block_id, assets);
        }
    }
}

/// An inline node and its descendants; a block inside inline content is refused.
fn inline(node: &Node) -> Result<Inline, Error> {
    let Kind::Inline(content) = &node.kind else {
        return Err(Error("block found inside an inline container".into()));
    };
    Ok(Inline {
        content: content.clone(),
        source_span: node.span,
        children: node.children.iter().map(inline).collect::<Result<_, _>>()?,
    })
}

/// The text an inline node reads as, its children rendered first.
pub(crate) fn render_inline(inline: &Inline) -> String {
    inline_text(
        &inline.content,
        &inline
            .children
            .iter()
            .map(render_inline)
            .collect::<String>(),
    )
}

/// A destination's asset status: a fragment, a remote URL, or else the supplied local status,
/// unchecked by default.
pub(crate) fn destination_status(destination: &str, supplied: Option<&AssetStatus>) -> AssetStatus {
    if destination.starts_with('#') {
        AssetStatus::Fragment
    } else if destination.starts_with("//")
        || destination
            .split('/')
            .next()
            .is_some_and(|s| s.contains(':'))
    {
        AssetStatus::Remote
    } else {
        supplied.cloned().unwrap_or(AssetStatus::Unchecked)
    }
}

#[cfg(test)]
mod tests {
    use crate::{CanonicalizeInput, ExtractorBlock, SourceSpan, canonicalize};

    /// An extractor block anchored to one Markdown span.
    fn extracted(id: &str, span: SourceSpan) -> ExtractorBlock {
        ExtractorBlock {
            extractor_id: id.into(),
            markdown_spans: vec![span],
            original_locations: Vec::new(),
            structured_content: serde_json::json!({}),
        }
    }

    #[test]
    fn extractor_blocks_attach_to_overlapping_blocks_not_touching_ones() {
        let markdown = "First\n\nSecond\n";
        let plain = canonicalize(CanonicalizeInput::new(markdown, "two.md")).unwrap();
        let [first, second] = [0, 1].map(|i| plain.blocks[i].source_spans[0]);
        assert!(first.end < second.start, "{first:?} {second:?}");
        let mut input = CanonicalizeInput::new(markdown, "two.md");
        input.extractor_blocks = vec![
            extracted("first", first),
            extracted(
                "between",
                SourceSpan {
                    start: first.end,
                    end: second.start,
                },
            ),
        ];
        let doc = canonicalize(input).unwrap();
        let attached: Vec<_> = doc.blocks.iter().map(|b| &b.extractor_block_ids).collect();
        assert_eq!(attached, [&vec!["first".to_owned()], &Vec::new()]);
    }
}
