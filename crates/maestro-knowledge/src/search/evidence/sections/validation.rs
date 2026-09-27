//! Validates canonical block links and source spans before evidence expansion.

use maestro_canonicalization::{
    Block, BlockAttributes, BlockType, CanonicalDocument, ContentNode, Section, SourceSpan,
};
use maestro_kernel::evidence::Span;
use std::collections::{BTreeMap, BTreeSet};

/// Returns the validated source envelope of a canonical block.
pub(in crate::search::evidence) fn block_span(
    block: &Block,
    markdown: &str,
) -> Result<Option<Span>, String> {
    let mut spans = block.source_spans.iter();
    let Some(first) = spans.next() else {
        return Ok(None);
    };
    if !valid_span(
        Span {
            start: first.start,
            end: first.end,
        },
        markdown,
    ) {
        return Err("canonical block has an invalid source span".to_owned());
    }
    let mut start = first.start;
    let mut end = first.end;
    for span in spans {
        if !valid_span(
            Span {
                start: span.start,
                end: span.end,
            },
            markdown,
        ) {
            return Err("canonical block has an invalid source span".to_owned());
        }
        start = start.min(span.start);
        end = end.max(span.end);
    }
    Ok(Some(Span { start, end }))
}

/// Reads a heading's canonical level, rejecting malformed heading attributes.
pub(super) fn heading_level(block: &Block) -> Result<Option<u8>, String> {
    if !matches!(&block.block_type, BlockType::Heading) {
        return Ok(None);
    }
    match &block.structured_content.attributes {
        BlockAttributes::Heading { level, .. } => Ok(Some(*level)),
        _ => Err("canonical heading has invalid attributes".to_owned()),
    }
}

/// Verifies canonical parent-child references and source containment.
pub(super) fn validate_block_links(
    document: &CanonicalDocument,
    blocks: &BTreeMap<String, &Block>,
    ranges: &BTreeMap<String, Span>,
) -> Result<(), String> {
    for block in &document.blocks {
        validate_child_links(block, blocks)?;
        validate_parent_link(block, blocks, ranges)?;
    }
    Ok(())
}

/// Checks child IDs and their reverse parent references.
fn validate_child_links(block: &Block, blocks: &BTreeMap<String, &Block>) -> Result<(), String> {
    for node in &block.structured_content.children {
        let ContentNode::Block { block_id } = node else {
            continue;
        };
        let child = blocks
            .get(block_id)
            .ok_or_else(|| "canonical block has a missing child link".to_owned())?;
        if child.parent_block_id.as_deref() != Some(block.block_id.as_str()) {
            return Err("canonical child and parent links disagree".to_owned());
        }
    }
    Ok(())
}

/// Checks the block's parent range and every ancestor link.
fn validate_parent_link(
    block: &Block,
    blocks: &BTreeMap<String, &Block>,
    ranges: &BTreeMap<String, Span>,
) -> Result<(), String> {
    let Some(parent_id) = &block.parent_block_id else {
        return Ok(());
    };
    if !blocks.contains_key(parent_id) {
        return Err("canonical block has a missing parent link".to_owned());
    }
    let child_span = ranges
        .get(&block.block_id)
        .ok_or_else(|| "canonical child block has no source span".to_owned())?;
    let parent_span = ranges
        .get(parent_id)
        .ok_or_else(|| "canonical parent block has no source span".to_owned())?;
    if !contains(*parent_span, *child_span) {
        return Err("canonical parent does not contain its child".to_owned());
    }
    let mut chain = BTreeSet::new();
    let mut current = Some(block.block_id.as_str());
    while let Some(id) = current {
        if !chain.insert(id) {
            return Err("canonical block links are cyclic".to_owned());
        }
        current = blocks
            .get(id)
            .and_then(|ancestor| ancestor.parent_block_id.as_deref());
    }
    Ok(())
}

/// Verifies that each canonical section parent chain is complete and acyclic.
pub(super) fn validate_section_links(
    document: &CanonicalDocument,
    sections: &BTreeMap<String, &Section>,
) -> Result<(), String> {
    for section in &document.sections {
        let mut chain = BTreeSet::new();
        let mut current = Some(section.section_id.as_str());
        while let Some(id) = current {
            if !chain.insert(id) {
                return Err("canonical section links are cyclic".to_owned());
            }
            let node = sections
                .get(id)
                .ok_or_else(|| "canonical section has a missing parent link".to_owned())?;
            current = node.parent_section_id.as_deref();
        }
    }
    Ok(())
}

/// Checks source bounds and UTF-8 boundaries for one span.
pub(in crate::search::evidence) fn valid_span(span: Span, markdown: &str) -> bool {
    SourceSpan {
        start: span.start,
        end: span.end,
    }
    .is_valid(markdown)
}

/// Tests half-open source-span containment.
pub(in crate::search::evidence) fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}
