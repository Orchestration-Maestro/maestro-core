//! Mapping a canonical document into source units: each block's context and inline text, with
//! the origins every byte of it comes from.
use super::refusal::invalid_mapping;
use super::slice::{map_accounting, mapped_slice};
use crate::{
    CanonicalDocument, Error,
    assemble::render_inline,
    chunks::{
        CHUNKER_VERSION, InlineEnvelope, MappedDocument, MappingRun, OriginMode, SourceOrigin,
        SourceUnit, TextRange, UnitField,
    },
    content::{Block, BlockAttributes, BlockType, ContentNode, Inline, InlineKind},
    digest,
    model::SourceSpan,
    parse,
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

/// Map a canonical document into source units, visiting every block once from the roots, and its
/// source ledger into dispositions.
pub(crate) fn map_document(
    document: &CanonicalDocument,
    markdown: &str,
) -> Result<MappedDocument, Error> {
    // Reuse the exact parser option profile for Unicode/case/whitespace reference lookup.
    let parser =
        pulldown_cmark::Parser::new_ext(markdown, parse::options(&document.parser_options));
    let mut mapper = Mapper {
        markdown,
        blocks: document
            .blocks
            .iter()
            .map(|block| (block.block_id.as_str(), block))
            .collect(),
        references: parser.reference_definitions(),
        visited: BTreeSet::new(),
        units: Vec::new(),
    };
    for block in document
        .blocks
        .iter()
        .filter(|block| block.parent_block_id.is_none())
    {
        mapper.walk(&block.block_id)?;
    }
    if mapper.visited.len() != document.blocks.len() {
        return Err(invalid_mapping());
    }
    let accounting = map_accounting(document, markdown, &mapper.units)?;
    Ok(MappedDocument {
        units: mapper.units,
        accounting,
    })
}

/// The walk over a document's blocks that builds its source units.
struct Mapper<'a> {
    /// The original Markdown every origin must lie in.
    markdown: &'a str,
    /// The document's blocks by identifier.
    blocks: BTreeMap<&'a str, &'a Block>,
    /// The Markdown's reference definitions, read with the document's own parser options.
    references: &'a pulldown_cmark::RefDefs<'a>,
    /// The blocks already walked; a second visit is a mapping error.
    visited: BTreeSet<&'a str>,
    /// The units built so far, in document order.
    units: Vec<SourceUnit>,
}

impl Mapper<'_> {
    /// Map a block and its descendants: its context unit first (code info, list marker, alert or
    /// note label), then its children in order, inline content as primary units.
    fn walk(&mut self, id: &str) -> Result<(), Error> {
        let block = self.blocks.get(id).copied().ok_or_else(invalid_mapping)?;
        if !self.visited.insert(&block.block_id) {
            return Err(invalid_mapping());
        }
        match &block.structured_content.attributes {
            BlockAttributes::Metadata
            | BlockAttributes::ReferenceDefinition { .. }
            | BlockAttributes::ThematicBreak => return Ok(()),
            BlockAttributes::Code {
                info: Some(info), ..
            } => self.context(block, UnitField::CodeInfo, info)?,
            BlockAttributes::ListItem => {
                let marker = self.list_marker(block)?;
                self.context(block, UnitField::ListMarker, &marker)?;
            }
            BlockAttributes::BlockQuote { alert: Some(alert) } => {
                self.context(block, UnitField::QuoteAlert, &format!("[!{alert}]"))?;
            }
            BlockAttributes::FootnoteDefinition { label } => {
                self.context(block, UnitField::FootnoteLabel, &format!("[^{label}]:"))?;
            }
            BlockAttributes::Heading { .. }
            | BlockAttributes::Paragraph
            | BlockAttributes::List { .. }
            | BlockAttributes::Code { info: None, .. }
            | BlockAttributes::Table { .. }
            | BlockAttributes::TableHead
            | BlockAttributes::TableRow
            | BlockAttributes::TableCell
            | BlockAttributes::BlockQuote { alert: None }
            | BlockAttributes::Html
            | BlockAttributes::DefinitionList
            | BlockAttributes::DefinitionTerm
            | BlockAttributes::DefinitionDescription
            | BlockAttributes::Raw { .. } => {}
        }
        for (index, child) in block.structured_content.children.iter().enumerate() {
            match child {
                ContentNode::Block { block_id } => self.walk(block_id)?,
                ContentNode::Inline { inline } => self.inline_unit(block, index, inline)?,
            }
        }
        Ok(())
    }

    /// A list item's canonical marker: its ordinal among its list's items after the list's start,
    /// or a dash in a bullet list.
    fn list_marker(&self, block: &Block) -> Result<String, Error> {
        let parent = self
            .blocks
            .get(
                block
                    .parent_block_id
                    .as_deref()
                    .ok_or_else(invalid_mapping)?,
            )
            .copied()
            .ok_or_else(invalid_mapping)?;
        let BlockAttributes::List { start } = parent.structured_content.attributes else {
            return Err(invalid_mapping());
        };
        let ordinal = parent
            .structured_content
            .children
            .iter()
            .filter_map(|child| match child {
                ContentNode::Block { block_id } => self.blocks.get(block_id.as_str()).copied(),
                ContentNode::Inline { .. } => None,
            })
            .filter(|block| block.block_type == BlockType::ListItem)
            .position(|candidate| candidate.block_id == block.block_id)
            .ok_or_else(invalid_mapping)?;
        Ok(match start {
            Some(start) => format!(
                "{}. ",
                u128::from(start) + u128::try_from(ordinal).map_err(|_| invalid_mapping())?
            ),
            None => "- ".into(),
        })
    }

    /// Map a block's inline child as a primary unit, whose text must be the inline's rendering.
    fn inline_unit(&mut self, block: &Block, index: usize, inline: &Inline) -> Result<(), Error> {
        let mut unit = new_unit(
            block,
            UnitField::Inline {
                child_path: vec![index],
            },
            true,
        )?;
        self.render(block, inline, &mut unit)?;
        if unit.text != render_inline(inline) {
            return Err(invalid_mapping());
        }
        unit.envelopes
            .sort_by_key(|envelope| (envelope.opening.start, Reverse(envelope.closing.end)));
        self.push(unit)
    }

    /// Keep a unit that has text, once its whole text maps back to its source.
    fn push(&mut self, unit: SourceUnit) -> Result<(), Error> {
        if !unit.text.is_empty() {
            mapped_slice(
                &unit,
                TextRange {
                    start: 0,
                    end: unit.text.len(),
                },
                self.markdown,
            )?;
            self.units.push(unit);
        }
        Ok(())
    }

    /// Add a non-primary context unit whose text comes from the whole block's source spans.
    fn context(&mut self, block: &Block, field: UnitField, text: &str) -> Result<(), Error> {
        let mut unit = new_unit(block, field, false)?;
        let origins = block
            .source_spans
            .iter()
            .map(|&span| SourceOrigin {
                block_id: block.block_id.clone(),
                span,
            })
            .collect();
        self.append(&mut unit, text, origins)?;
        self.push(unit)
    }

    /// Append text with its origins: an exact copy when it equals its single source span, a
    /// canonical transformation otherwise.
    fn append(
        &self,
        unit: &mut SourceUnit,
        text: &str,
        origins: Vec<SourceOrigin>,
    ) -> Result<(), Error> {
        if text.is_empty() {
            return Ok(());
        }
        if origins.is_empty()
            || origins
                .iter()
                .any(|origin| !origin.span.is_valid(self.markdown))
        {
            return Err(invalid_mapping());
        }
        let mode = match origins.as_slice() {
            [origin] if self.markdown.get(origin.span.start..origin.span.end) == Some(text) => {
                OriginMode::ExactCopy
            }
            _ => OriginMode::CanonicalTransformation,
        };
        let start = unit.text.len();
        unit.text.push_str(text);
        unit.mappings.push(MappingRun {
            range: TextRange {
                start,
                end: unit.text.len(),
            },
            mode,
            origins,
        });
        Ok(())
    }

    /// Render an inline node into a unit's text and mappings; deletion and script delimiters become
    /// envelopes around their content.
    fn render(&self, block: &Block, inline: &Inline, unit: &mut SourceUnit) -> Result<(), Error> {
        let origin = SourceOrigin {
            block_id: block.block_id.clone(),
            span: inline.source_span,
        };
        match &inline.content {
            InlineKind::Emphasis | InlineKind::Strong | InlineKind::Link { .. } => {
                for child in &inline.children {
                    self.render(block, child, unit)?;
                }
            }
            InlineKind::Strikethrough | InlineKind::Superscript | InlineKind::Subscript => {
                let (opening, closing) = match inline.content {
                    InlineKind::Strikethrough => ("~~", "~~"),
                    InlineKind::Superscript => ("^(", ")"),
                    _ => ("_(", ")"),
                };
                let start = unit.text.len();
                self.append(unit, opening, vec![origin.clone()])?;
                let opening = TextRange {
                    start,
                    end: unit.text.len(),
                };
                for child in &inline.children {
                    self.render(block, child, unit)?;
                }
                let start = unit.text.len();
                self.append(unit, closing, vec![origin])?;
                unit.envelopes.push(InlineEnvelope {
                    opening,
                    closing: TextRange {
                        start,
                        end: unit.text.len(),
                    },
                });
            }
            InlineKind::Image {
                destination,
                reference,
                ..
            } => {
                for child in &inline.children {
                    self.render(block, child, unit)?;
                }
                let mut origins = vec![origin];
                if !reference.is_empty() {
                    origins.push(self.definition_origin(reference)?);
                }
                self.append(unit, &format!(" ({destination})"), origins)?;
            }
            InlineKind::Text { .. }
            | InlineKind::Code { .. }
            | InlineKind::Math { .. }
            | InlineKind::Html { .. }
            | InlineKind::FootnoteReference { .. }
            | InlineKind::SoftBreak
            | InlineKind::HardBreak
            | InlineKind::TaskMarker { .. } => {
                self.append(unit, &parse::inline_text(&inline.content, ""), vec![origin])?;
            }
        }
        Ok(())
    }

    /// The origin of a reference image's resolved definition: its span, owned by the reference
    /// definition block that holds it.
    fn definition_origin(&self, reference: &str) -> Result<SourceOrigin, Error> {
        let definition = self.references.get(reference).ok_or_else(invalid_mapping)?;
        let span = SourceSpan {
            start: definition.span.start,
            end: definition.span.end,
        };
        // ponytail: linear definition lookup; index spans if image-heavy
        // documents make this material.
        let owner = self
            .blocks
            .values()
            .find(|candidate| {
                candidate.block_type == BlockType::ReferenceDefinition
                    && candidate.source_spans.contains(&span)
            })
            .ok_or_else(invalid_mapping)?;
        Ok(SourceOrigin {
            block_id: owner.block_id.clone(),
            span,
        })
    }
}

/// An empty unit of a block field, its identifier derived from the chunker version, the block and
/// the field.
fn new_unit(block: &Block, field: UnitField, primary: bool) -> Result<SourceUnit, Error> {
    let bytes = serde_json::to_vec(&(CHUNKER_VERSION, &block.block_id, &field))
        .map_err(|_| invalid_mapping())?;
    Ok(SourceUnit {
        unit_id: format!("unit-{}", digest(&bytes)),
        block_id: block.block_id.clone(),
        field,
        primary,
        text: String::new(),
        mappings: Vec::new(),
        envelopes: Vec::new(),
    })
}
