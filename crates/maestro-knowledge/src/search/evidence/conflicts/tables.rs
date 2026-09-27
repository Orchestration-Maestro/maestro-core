//! Reads explicit entity/attribute/value facts from supported Markdown tables.

use maestro_canonicalization::{
    Block, BlockAttributes, BlockType, CanonicalDocument, ContentNode, Section,
};
use maestro_kernel::evidence::Span;
use std::collections::BTreeMap;

use super::super::{features::has_condition_or_negation, sections::block_span};

/// Canonical identity for one row observation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct FactKey {
    /// Source revision containing the table.
    pub(super) revision_id: String,
    /// Canonical table block ID.
    pub(super) table_id: String,
    /// Zero-based row position in the table.
    pub(super) row_position: usize,
}

/// A supported entity/attribute/value row and its canonical location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TableFact {
    /// Stable row identity used to deduplicate overlapping candidates.
    pub(super) key: FactKey,
    /// Full source span of the owning table block.
    pub(super) table_span: Span,
    /// Heading path of the table's canonical section.
    pub(super) section_path: Vec<String>,
    /// One-based occurrence of the heading path.
    pub(super) occurrence: usize,
    /// Entity or parameter value after edge trimming.
    pub(super) entity: String,
    /// Attribute name or the fixed `default` name.
    pub(super) attribute: String,
    /// Explicit value after edge trimming.
    pub(super) value: String,
}

/// One of the exact table header shapes accepted for conflict detection.
#[derive(Clone, Copy)]
enum TableShape {
    /// Three-column entity, attribute, and value table.
    EntityAttributeValue,
    /// Two-column parameter and default table.
    ParameterDefault,
}

/// Parsed row position and its entity, attribute, and value.
type ParsedRow = (usize, String, String, String);

/// Parsed entity, attribute, and value cells.
type ParsedValues = (String, String, String);

/// Extracts supported table facts from canonical table structure.
pub(super) fn table_facts(
    document: &CanonicalDocument,
    markdown: &str,
) -> Result<Vec<TableFact>, String> {
    let blocks: BTreeMap<_, _> = document
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block))
        .collect();
    if blocks.len() != document.blocks.len() {
        return Err("canonical block IDs are not unique".to_owned());
    }
    let sections = section_occurrences(document.sections.iter())?;
    let mut facts = Vec::new();
    for table in &document.blocks {
        if !matches!(
            &table.structured_content.attributes,
            BlockAttributes::Table { .. }
        ) {
            continue;
        }
        let span = block_span(table, markdown)?
            .ok_or_else(|| "canonical table has no source span".to_owned())?;
        let (section_path, occurrence) = match table.parent_section_id.as_deref() {
            Some(section_id) => sections
                .get(section_id)
                .cloned()
                .ok_or_else(|| "canonical table names a missing section".to_owned())?,
            None => (Vec::new(), 1),
        };
        if let Some(rows) = parse_table(table, &blocks)? {
            facts.extend(
                rows.into_iter()
                    .map(|(row_position, entity, attribute, value)| TableFact {
                        key: FactKey {
                            revision_id: document.revision_id.clone(),
                            table_id: table.block_id.clone(),
                            row_position,
                        },
                        table_span: span,
                        section_path: section_path.clone(),
                        occurrence,
                        entity,
                        attribute,
                        value,
                    }),
            );
        }
    }
    Ok(facts)
}

/// Assigns each canonical heading path its one-based occurrence number.
fn section_occurrences<'a>(
    sections: impl Iterator<Item = &'a Section>,
) -> Result<BTreeMap<String, (Vec<String>, usize)>, String> {
    let mut counts: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    let mut output = BTreeMap::new();
    for section in sections {
        let count = counts.entry(section.heading_path.clone()).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| "section occurrence exceeds usize".to_owned())?;
        if output
            .insert(
                section.section_id.clone(),
                (section.heading_path.clone(), *count),
            )
            .is_some()
        {
            return Err("canonical section IDs are not unique".to_owned());
        }
    }
    Ok(output)
}

/// Parses one exact supported table shape and its well-formed rows.
fn parse_table(
    table: &Block,
    blocks: &BTreeMap<&str, &Block>,
) -> Result<Option<Vec<ParsedRow>>, String> {
    let children = child_blocks(table, blocks)?;
    if children.len() != table.structured_content.children.len() {
        return Ok(None);
    }
    let headers: Vec<_> = children
        .iter()
        .copied()
        .filter(|child| matches!(&child.block_type, BlockType::TableHead))
        .collect();
    if headers.len() != 1
        || children.iter().any(|child| {
            !matches!(
                &child.block_type,
                BlockType::TableHead | BlockType::TableRow
            )
        })
    {
        return Ok(None);
    }
    let Some(header_row) = headers.first().copied() else {
        return Ok(None);
    };
    let header_cells = child_blocks(header_row, blocks)?;
    if header_cells.len() != header_row.structured_content.children.len()
        || header_cells
            .iter()
            .any(|cell| !matches!(&cell.block_type, BlockType::TableCell))
    {
        return Ok(None);
    }
    let mut header = Vec::with_capacity(header_cells.len());
    for cell in &header_cells {
        let Some(text) = cell_text(cell) else {
            return Ok(None);
        };
        header.push(text.trim().to_ascii_lowercase());
    }
    let shape = match header.as_slice() {
        [entity, attribute, value]
            if [entity.as_str(), attribute.as_str(), value.as_str()]
                == ["entity", "attribute", "value"] =>
        {
            TableShape::EntityAttributeValue
        }
        [parameter, default]
            if [parameter.as_str(), default.as_str()] == ["parameter", "default"] =>
        {
            TableShape::ParameterDefault
        }
        _ => return Ok(None),
    };
    let rows: Vec<_> = children
        .into_iter()
        .filter(|child| matches!(&child.block_type, BlockType::TableRow))
        .collect();
    let mut parsed = Vec::with_capacity(rows.len());
    for (row_position, row) in rows.into_iter().enumerate() {
        if has_condition_or_negation(&row.retrieval_text) {
            return Ok(None);
        }
        let cells = child_blocks(row, blocks)?;
        if cells.len() != row.structured_content.children.len()
            || cells.len() != header.len()
            || cells
                .iter()
                .any(|cell| !matches!(&cell.block_type, BlockType::TableCell))
        {
            return Ok(None);
        }
        let Some(values) = cells
            .iter()
            .map(|cell| cell_text(cell).map(str::to_owned))
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(None);
        };
        if values.iter().any(|value| value.trim().is_empty()) {
            return Ok(None);
        }
        let Some((entity, attribute, value)) = parse_row_values(shape, &values) else {
            return Ok(None);
        };
        parsed.push((row_position, entity, attribute, value));
    }
    Ok(Some(parsed))
}

/// Trims the supported cell values for one validated header shape.
fn parse_row_values(shape: TableShape, values: &[String]) -> Option<ParsedValues> {
    match shape {
        TableShape::EntityAttributeValue => Some((
            values.first()?.trim().to_owned(),
            values.get(1)?.trim().to_owned(),
            values.get(2)?.trim().to_owned(),
        )),
        TableShape::ParameterDefault => Some((
            values.first()?.trim().to_owned(),
            "default".to_owned(),
            values.get(1)?.trim().to_owned(),
        )),
    }
}

/// Resolves direct canonical child blocks, refusing missing block references.
fn child_blocks<'a>(
    parent: &'a Block,
    blocks: &BTreeMap<&'a str, &'a Block>,
) -> Result<Vec<&'a Block>, String> {
    parent
        .structured_content
        .children
        .iter()
        .filter_map(|node| match node {
            ContentNode::Block { block_id } => Some(
                blocks
                    .get(block_id.as_str())
                    .copied()
                    .ok_or_else(|| "canonical table has a missing child block".to_owned()),
            ),
            ContentNode::Inline { .. } => None,
        })
        .collect()
}

/// Reads a cell's inline text only when it has no nested block content.
fn cell_text(cell: &Block) -> Option<&str> {
    if cell
        .structured_content
        .children
        .iter()
        .any(|child| matches!(child, ContentNode::Block { .. }))
    {
        return None;
    }
    Some(&cell.retrieval_text)
}
