//! Derives validated section extents and bounded sibling windows.

use super::super::spans::SpanUnion;
use super::validation::{
    block_span, block_span_with_length, contains, heading_level, validate_block_links,
    validate_section_links,
};
use maestro_canonicalization::{Block, BlockType, CanonicalDocument, Section};
use maestro_kernel::evidence::Span;
use std::collections::BTreeMap;

/// One canonical block's validated source range and lexical parent.
#[derive(Debug, Clone)]
struct BlockRange {
    /// Stable canonical block ID.
    id: String,
    /// Immediate lexical container, if nested.
    parent_block_id: Option<String>,
    /// Canonical block kind.
    kind: BlockType,
    /// Heading level when this block is a heading.
    heading_level: Option<u8>,
    /// Envelope of the block's original-source spans.
    span: Span,
    /// Position in the canonical block arena.
    order: usize,
}

/// A canonical section's extent and the whole lexical siblings it contains.
#[derive(Debug, Clone)]
struct SectionRange {
    /// Section identity, equal to its heading block ID.
    id: String,
    /// Canonical heading path.
    path: Vec<String>,
    /// Half-open range ending before its next equal-or-shallower sibling.
    extent: Span,
    /// Heading's source position for deterministic ties.
    heading_start: usize,
    /// Whole lexical sibling blocks available for safe windows.
    siblings: Vec<Span>,
}

/// Validated section and root-content ranges for one canonical document.
#[derive(Debug)]
pub(crate) struct SectionIndex {
    /// Canonical sections in document order.
    sections: Vec<SectionRange>,
    /// Root-level non-frontmatter blocks.
    root_content: Vec<Span>,
    /// Root-level preamble blocks before the first heading.
    root_preamble: Vec<Span>,
    /// Envelope of all root-level content.
    content_extent: Option<Span>,
    /// Envelope of root-level preamble content.
    preamble_extent: Option<Span>,
}

/// Full section or root-content extent selected for a seed union.
#[derive(Debug, Clone)]
pub(crate) struct Expansion {
    /// Chosen canonical section, absent for document-level evidence.
    pub(crate) section_id: Option<String>,
    /// Chosen section heading path.
    pub(crate) section_path: Vec<String>,
    /// Complete enclosing source extent.
    pub(crate) extent: Span,
    /// Whole lexical siblings inside the selected extent.
    siblings: Vec<Span>,
}

/// A mandatory whole-sibling core and optional neighbors on each side.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct WindowPlan {
    /// Smallest consecutive siblings containing the seed.
    pub(crate) mandatory: Span,
    /// Up to two predecessors, nearest first.
    pub(crate) before: Vec<Span>,
    /// Up to two successors, nearest first.
    pub(crate) after: Vec<Span>,
}

/// Canonical block identities and their validated source ranges.
struct BlockIndex<'a> {
    /// All source-bearing blocks in canonical arena order.
    ranges: Vec<BlockRange>,
    /// Every canonical block, including blocks without a source span.
    blocks: BTreeMap<String, &'a Block>,
    /// Source spans indexed by canonical block ID.
    spans: BTreeMap<String, Span>,
}

/// Root-level content and preamble source ranges.
struct RootRanges {
    /// Source ranges of all root-level content blocks.
    content: Vec<Span>,
    /// Source ranges before the first root-level heading.
    preamble: Vec<Span>,
    /// Envelope of all root-level content.
    content_extent: Option<Span>,
    /// Envelope of the root-level preamble.
    preamble_extent: Option<Span>,
}

/// Builds a block index while validating all canonical source spans.
fn block_index<'a>(
    document: &'a CanonicalDocument,
    source_length: usize,
    markdown: Option<&str>,
) -> Result<BlockIndex<'a>, String> {
    let mut ranges = Vec::with_capacity(document.blocks.len());
    let mut blocks = BTreeMap::new();
    for (order, block) in document.blocks.iter().enumerate() {
        if blocks.insert(block.block_id.clone(), block).is_some() {
            return Err("canonical block IDs are not unique".to_owned());
        }
        let Some(span) = (match markdown {
            Some(markdown) => block_span(block, markdown)?,
            None => block_span_with_length(block, source_length)?,
        }) else {
            continue;
        };
        ranges.push(BlockRange {
            id: block.block_id.clone(),
            parent_block_id: block.parent_block_id.clone(),
            kind: block.block_type.clone(),
            heading_level: heading_level(block)?,
            span,
            order,
        });
    }
    let spans = ranges
        .iter()
        .map(|range| (range.id.clone(), range.span))
        .collect();
    Ok(BlockIndex {
        ranges,
        blocks,
        spans,
    })
}

/// Validates section links and derives each section's lexical extent.
fn section_ranges(
    document: &CanonicalDocument,
    blocks: &BlockIndex<'_>,
) -> Result<Vec<SectionRange>, String> {
    let mut sections = BTreeMap::new();
    for section in &document.sections {
        if sections
            .insert(section.section_id.clone(), section)
            .is_some()
        {
            return Err("canonical section IDs are not unique".to_owned());
        }
    }
    validate_section_links(document, &sections)?;
    if document.blocks.iter().any(|block| {
        block
            .parent_section_id
            .as_ref()
            .is_some_and(|id| !sections.contains_key(id))
    }) {
        return Err("canonical block names a missing section".to_owned());
    }
    document
        .sections
        .iter()
        .map(|section| section_range(section, blocks))
        .collect()
}

/// Derives a heading's range through its next equal-or-shallower sibling.
fn section_range(section: &Section, blocks: &BlockIndex<'_>) -> Result<SectionRange, String> {
    let heading = blocks
        .blocks
        .get(&section.section_id)
        .copied()
        .ok_or_else(|| "canonical section has no heading block".to_owned())?;
    if !matches!(&heading.block_type, BlockType::Heading)
        || heading.parent_section_id != section.parent_section_id
        || heading_level(heading)? != Some(section.level)
    {
        return Err("canonical section and heading disagree".to_owned());
    }
    let heading_range = blocks
        .ranges
        .iter()
        .find(|range| range.id == section.section_id)
        .ok_or_else(|| "canonical section heading has no source span".to_owned())?;
    let mut siblings: Vec<_> = blocks
        .ranges
        .iter()
        .filter(|range| range.parent_block_id == heading_range.parent_block_id)
        .collect();
    siblings.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then_with(|| left.order.cmp(&right.order))
    });
    let heading_position = siblings
        .iter()
        .position(|range| range.id == section.section_id)
        .ok_or_else(|| "canonical section heading is not its lexical sibling".to_owned())?;
    section_range_from_siblings(section, heading_range, &siblings, heading_position)
}

/// Builds a section extent from its ordered lexical siblings.
fn section_range_from_siblings(
    section: &Section,
    heading: &BlockRange,
    siblings: &[&BlockRange],
    heading_position: usize,
) -> Result<SectionRange, String> {
    let mut selected = vec![heading.span];
    let mut end = heading.span.end;
    for sibling in siblings.iter().skip(heading_position.saturating_add(1)) {
        if sibling
            .heading_level
            .is_some_and(|level| level <= section.level)
        {
            end = sibling.span.start;
            break;
        }
        selected.push(sibling.span);
        end = end.max(sibling.span.end);
    }
    if end <= heading.span.start || selected.iter().any(|span| span.end > end) {
        return Err("canonical section extent is invalid".to_owned());
    }
    Ok(SectionRange {
        id: section.section_id.clone(),
        path: section.heading_path.clone(),
        extent: Span {
            start: heading.span.start,
            end,
        },
        heading_start: heading.span.start,
        siblings: selected,
    })
}

/// Derives root content and the leading preamble from source-bearing blocks.
fn root_ranges(ranges: &[BlockRange]) -> RootRanges {
    let mut root: Vec<_> = ranges
        .iter()
        .filter(|range| {
            range.parent_block_id.is_none() && !matches!(&range.kind, BlockType::Metadata)
        })
        .collect();
    root.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then_with(|| left.order.cmp(&right.order))
    });
    let content: Vec<_> = root.iter().map(|range| range.span).collect();
    let content_extent = extent_of(&content);
    let first_heading = root
        .iter()
        .find(|range| range.heading_level.is_some())
        .map(|range| range.span.start);
    let preamble: Vec<_> = root
        .iter()
        .take_while(|range| first_heading.is_none_or(|start| range.span.start < start))
        .map(|range| range.span)
        .collect();
    let preamble_extent = match (first_heading, extent_of(&preamble)) {
        (Some(heading_start), Some(preamble)) => Some(Span {
            start: preamble.start,
            end: heading_start,
        }),
        (None, extent) => extent,
        (Some(_), None) => None,
    };
    RootRanges {
        content,
        preamble,
        content_extent,
        preamble_extent,
    }
}

impl SectionIndex {
    /// Builds validated lexical ranges from canonical blocks and source bytes.
    pub(crate) fn new(document: &CanonicalDocument, markdown: &str) -> Result<Self, String> {
        Self::from_source(document, markdown.len(), Some(markdown))
    }

    /// Builds extent ranges using source length before loading source text.
    pub(crate) fn new_from_length(
        document: &CanonicalDocument,
        source_length: usize,
    ) -> Result<Self, String> {
        Self::from_source(document, source_length, None)
    }

    /// Builds lexical ranges, optionally checking UTF-8 boundaries in source text.
    fn from_source(
        document: &CanonicalDocument,
        source_length: usize,
        markdown: Option<&str>,
    ) -> Result<Self, String> {
        if markdown.is_some_and(|source| source.len() != source_length) {
            return Err("canonical source length does not match Markdown".to_owned());
        }
        let blocks = block_index(document, source_length, markdown)?;
        validate_block_links(document, &blocks.blocks, &blocks.spans)?;
        let sections = section_ranges(document, &blocks)?;
        let root = root_ranges(&blocks.ranges);
        Ok(Self {
            sections,
            root_content: root.content,
            root_preamble: root.preamble,
            content_extent: root.content_extent,
            preamble_extent: root.preamble_extent,
        })
    }

    /// Returns the authoritative source extent for a canonical section ID.
    pub(crate) fn section_extent(&self, section_id: &str) -> Option<Span> {
        self.sections
            .iter()
            .find(|section| section.id == section_id)
            .map(|section| section.extent)
    }

    /// Chooses the deepest section containing a union, otherwise root content.
    pub(crate) fn expand(&self, union: &SpanUnion) -> Result<Expansion, String> {
        if union.span.start >= union.span.end
            || union
                .seeds
                .iter()
                .any(|seed| seed.revision_id != union.revision_id)
        {
            return Err("seed union is invalid".to_owned());
        }
        // A chunk's source envelope can include a reference definition outside its section.
        for seed in &union.seeds {
            let Some(section_id) = &seed.section_id else {
                continue;
            };
            if self.section_extent(section_id).is_none() {
                return Err("candidate names a missing canonical section".to_owned());
            }
        }

        let mut selected: Option<&SectionRange> = None;
        for section in &self.sections {
            if contains(section.extent, union.span)
                && selected.is_none_or(|previous| {
                    section.heading_start > previous.heading_start
                        || (section.heading_start == previous.heading_start
                            && section.extent.end < previous.extent.end)
                })
            {
                selected = Some(section);
            }
        }
        if let Some(section) = selected {
            return Ok(Expansion {
                section_id: Some(section.id.clone()),
                section_path: section.path.clone(),
                extent: section.extent,
                siblings: section.siblings.clone(),
            });
        }

        let sectionless = union.seeds.iter().all(|seed| seed.section_id.is_none());
        let preamble_contains = self
            .preamble_extent
            .is_some_and(|extent| contains(extent, union.span));
        let (extent, siblings) = if sectionless && preamble_contains {
            (self.preamble_extent, &self.root_preamble)
        } else {
            (self.content_extent, &self.root_content)
        };
        let extent = extent.ok_or_else(|| "source has no enclosing content extent".to_owned())?;
        if !contains(extent, union.span) {
            return Err("content extent does not contain its seed union".to_owned());
        }
        Ok(Expansion {
            section_id: None,
            section_path: Vec::new(),
            extent,
            siblings: siblings.clone(),
        })
    }
}

impl Expansion {
    /// Whether a final source span omits any sibling in the chosen expansion.
    pub(crate) fn is_windowed(&self, span: Span) -> bool {
        span != self.extent
            || self
                .siblings
                .iter()
                .any(|sibling| !contains(span, *sibling))
    }

    /// Builds a mandatory whole-sibling run and at most four neighbors.
    pub(crate) fn window_plan(&self, seed: Span) -> Result<WindowPlan, String> {
        if seed.start >= seed.end || !contains(self.extent, seed) {
            return Err("window seed is outside its expansion extent".to_owned());
        }
        let first = self
            .siblings
            .iter()
            .position(|span| span.start <= seed.start && seed.start < span.end)
            .ok_or_else(|| "no sibling contains the window start".to_owned())?;
        let last = self
            .siblings
            .iter()
            .rposition(|span| span.start < seed.end && seed.end <= span.end)
            .ok_or_else(|| "no sibling contains the window end".to_owned())?;
        if first > last {
            return Err("window sibling order is invalid".to_owned());
        }
        let first_span = self
            .siblings
            .get(first)
            .copied()
            .ok_or_else(|| "window start sibling disappeared".to_owned())?;
        let last_span = self
            .siblings
            .get(last)
            .copied()
            .ok_or_else(|| "window end sibling disappeared".to_owned())?;
        let mandatory = Span {
            start: first_span.start,
            end: last_span.end,
        };
        if !contains(mandatory, seed) {
            return Err("mandatory sibling run does not contain its seed".to_owned());
        }
        let mut before = Vec::with_capacity(2);
        if let Some(previous) = first
            .checked_sub(1)
            .and_then(|index| self.siblings.get(index))
        {
            before.push(*previous);
        }
        if let Some(previous) = first
            .checked_sub(2)
            .and_then(|index| self.siblings.get(index))
        {
            before.push(*previous);
        }
        let mut after = Vec::with_capacity(2);
        if let Some(next) = last
            .checked_add(1)
            .and_then(|index| self.siblings.get(index))
        {
            after.push(*next);
        }
        if let Some(next) = last
            .checked_add(2)
            .and_then(|index| self.siblings.get(index))
        {
            after.push(*next);
        }
        Ok(WindowPlan {
            mandatory,
            before,
            after,
        })
    }
}

/// Returns the envelope of a nonempty span collection.
fn extent_of(spans: &[Span]) -> Option<Span> {
    let start = spans.iter().map(|span| span.start).min()?;
    let end = spans.iter().map(|span| span.end).max()?;
    Some(Span { start, end })
}

#[cfg(test)]
mod tests {
    use super::SectionIndex;
    use maestro_canonicalization::{CanonicalizeInput, canonicalize};

    #[test]
    fn from_source_rejects_a_length_that_disagrees_with_text() {
        let markdown = "## Guide\n";
        let document = canonicalize(CanonicalizeInput::new(markdown, "fixture.md")).unwrap();

        assert!(SectionIndex::from_source(&document, markdown.len() + 1, Some(markdown)).is_err());
    }
}
