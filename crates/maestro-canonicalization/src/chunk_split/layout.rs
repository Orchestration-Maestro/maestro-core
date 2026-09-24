//! The layout's structural queries: owners, sections, table windows and packing atoms.
use super::prepare::{boundaries, fit_prefix, normalized_range};
use super::{Body, Layout, MAX_TOKENS, structure_error};
use crate::{
    Block, BlockType, ContentNode, Error,
    chunks::{ChunkContent, Contribution, Fragment, SplitKind, TableWindow, TextRange},
};

impl Layout<'_> {
    /// The block that owns a unit, the last of its ancestry; an unknown unit is a structure error.
    pub(super) fn owner(&self, unit: usize) -> Result<&Block, Error> {
        self.ancestry
            .get(unit)
            .and_then(|a| a.last())
            .copied()
            .ok_or_else(structure_error)
    }

    /// The innermost ancestor of a unit with the given block type.
    pub(super) fn nearest(&self, unit: usize, kind: &BlockType) -> Option<&Block> {
        self.ancestry[unit]
            .iter()
            .rev()
            .copied()
            .find(|b| b.block_type == *kind)
    }

    /// The heading whose section holds a unit: the unit's own heading, or its owner's section.
    pub(super) fn section(&self, unit: usize) -> Option<String> {
        let owner = self.ancestry[unit].last()?;
        if owner.block_type == BlockType::Heading {
            Some(owner.block_id.clone())
        } else {
            owner.parent_section_id.clone()
        }
    }

    /// What two units must share to share a chunk: their section and their enclosing containers
    /// (lists, code, tables, quotes, notes, definition lists, HTML and raw source).
    fn key(&self, unit: usize) -> (Option<String>, Vec<String>) {
        let containers = self.ancestry[unit]
            .iter()
            .filter(|b| {
                matches!(
                    b.block_type,
                    BlockType::List
                        | BlockType::Code
                        | BlockType::Table
                        | BlockType::BlockQuote
                        | BlockType::FootnoteDefinition
                        | BlockType::DefinitionList
                        | BlockType::Html
                        | BlockType::Raw
                )
            })
            .map(|b| b.block_id.clone())
            .collect();
        (self.section(unit), containers)
    }

    /// A block's child blocks in order; inline children are skipped.
    pub(super) fn child_blocks(&self, block: &Block) -> Vec<&Block> {
        block
            .structured_content
            .children
            .iter()
            .filter_map(|node| match node {
                ContentNode::Block { block_id } => self.blocks.get(block_id.as_str()).copied(),
                ContentNode::Inline { .. } => None,
            })
            .collect()
    }

    /// The cells of a table row, in column order.
    pub(super) fn row_cells(&self, row_id: &str) -> Result<Vec<&Block>, Error> {
        Ok(self
            .child_blocks(
                self.blocks
                    .get(row_id)
                    .copied()
                    .ok_or_else(structure_error)?,
            )
            .into_iter()
            .filter(|b| b.block_type == BlockType::TableCell)
            .collect())
    }

    /// The table window of a unit inside a table: its table, row and row index, with its own column
    /// or the whole row.
    pub(super) fn window(
        &self,
        unit: usize,
        whole_row: bool,
    ) -> Result<Option<TableWindow>, Error> {
        let Some(table) = self.nearest(unit, &BlockType::Table) else {
            return Ok(None);
        };
        let row = self.ancestry[unit]
            .iter()
            .rev()
            .copied()
            .find(|b| matches!(b.block_type, BlockType::TableHead | BlockType::TableRow))
            .ok_or_else(structure_error)?;
        let cell = self
            .nearest(unit, &BlockType::TableCell)
            .ok_or_else(structure_error)?;
        let rows: Vec<_> = self
            .child_blocks(table)
            .into_iter()
            .filter(|b| matches!(b.block_type, BlockType::TableHead | BlockType::TableRow))
            .collect();
        let cells = self.row_cells(&row.block_id)?;
        let column = cells
            .iter()
            .position(|b| b.block_id == cell.block_id)
            .ok_or_else(structure_error)?;
        Ok(Some(TableWindow {
            table_id: table.block_id.clone(),
            row_id: row.block_id.clone(),
            row_index: rows
                .iter()
                .position(|b| b.block_id == row.block_id)
                .ok_or_else(structure_error)?,
            columns: if whole_row {
                (0..cells.len()).collect()
            } else {
                vec![column]
            },
        }))
    }

    /// The packing atoms: runs of consecutive primary units that share a table row, list item or
    /// definition (else a block) and a key.
    pub(super) fn atoms(&self) -> Result<Vec<Body>, Error> {
        let mut atoms: Vec<Body> = Vec::new();
        let mut previous = None;
        for (index, unit) in self
            .mapped
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.primary)
        {
            let window = self.window(index, true)?;
            let natural = window.as_ref().map_or_else(
                || {
                    self.ancestry[index]
                        .iter()
                        .rev()
                        .find(|block| {
                            matches!(
                                block.block_type,
                                BlockType::ListItem | BlockType::DefinitionDescription
                            )
                        })
                        .map_or_else(|| unit.block_id.clone(), |item| item.block_id.clone())
                },
                |window| window.row_id.clone(),
            );
            let group = (natural, self.key(index));
            let fragment = Fragment {
                contribution: Contribution {
                    unit_index: index,
                    range: TextRange {
                        start: 0,
                        end: unit.text.len(),
                    },
                },
                part_ordinal: 0,
                split: SplitKind::Whole,
            };
            if previous.as_ref() == Some(&group) {
                atoms
                    .last_mut()
                    .ok_or_else(structure_error)?
                    .fragments
                    .push(fragment);
            } else {
                atoms.push(Body {
                    fragments: vec![fragment],
                    windows: window.into_iter().collect(),
                });
                previous = Some(group);
            }
        }
        Ok(atoms)
    }

    /// Whether two bodies may share a chunk: the same key, and either no table or one table with
    /// the same row or the same columns.
    pub(super) fn compatible(&self, left: &Body, right: &Body) -> bool {
        if self.key(left.fragments[0].contribution.unit_index)
            != self.key(right.fragments[0].contribution.unit_index)
        {
            return false;
        }
        match (left.windows.last(), right.windows.first()) {
            (Some(a), Some(b)) => {
                a.table_id == b.table_id && (a.row_id == b.row_id || a.columns == b.columns)
            }
            (None, None) => true,
            _ => false,
        }
    }

    /// Split an oversized body at its structure: a multi-column row into one body per column, else
    /// each fragment into its own body; nothing for a single fragment.
    pub(super) fn refine(&self, body: &Body) -> Result<Option<Vec<Body>>, Error> {
        if let Some(window) = body.windows.first().filter(|w| w.columns.len() > 1) {
            let cells = self.row_cells(&window.row_id)?;
            let mut result = Vec::new();
            for &column in &window.columns {
                let cell = cells.get(column).ok_or_else(structure_error)?;
                let fragments: Vec<_> = body
                    .fragments
                    .iter()
                    .filter(|f| {
                        self.mapped.units[f.contribution.unit_index].block_id == cell.block_id
                    })
                    .cloned()
                    .map(|mut f| {
                        f.split = SplitKind::Structural;
                        f
                    })
                    .collect();
                if !fragments.is_empty() {
                    let mut selected = window.clone();
                    selected.columns = vec![column];
                    result.push(Body {
                        fragments,
                        windows: vec![selected],
                    });
                }
            }
            if !result.is_empty() {
                return Ok(Some(result));
            }
        }
        if body.fragments.len() > 1 {
            let mut result = Vec::new();
            for fragment in &body.fragments {
                let mut fragment = fragment.clone();
                fragment.split = if self
                    .nearest(fragment.contribution.unit_index, &BlockType::Code)
                    .is_some()
                {
                    SplitKind::CodeLine
                } else {
                    SplitKind::Structural
                };
                result.push(Body {
                    windows: self
                        .window(fragment.contribution.unit_index, false)?
                        .into_iter()
                        .collect(),
                    fragments: vec![fragment],
                });
            }
            return Ok(Some(result));
        }
        Ok(None)
    }

    /// Split one oversized unit into chunks, each the longest prefix that fits, preferring sentence
    /// ends or code lines, then whitespace; each piece records how it was cut.
    pub(super) fn split_unit(
        &self,
        body: &Body,
        count: &mut impl FnMut(&str) -> Result<usize, Error>,
    ) -> Result<Vec<ChunkContent>, Error> {
        let original = body.fragments.first().ok_or_else(structure_error)?;
        let index = original.contribution.unit_index;
        let unit = &self.mapped.units[index];
        let code = self.nearest(index, &BlockType::Code).is_some();
        let cell = self.nearest(index, &BlockType::TableCell).is_some();
        let mut start = original.contribution.range.start;
        let stop = original.contribution.range.end;
        let mut result = Vec::new();
        while start < stop {
            let text = unit.text.get(start..stop).ok_or_else(structure_error)?;
            let (meaningful, whitespace) = boundaries(text, code);
            let preferred = if meaningful.is_empty() {
                &whitespace
            } else {
                &meaningful
            };
            let make = |length| -> Option<Body> {
                let range = normalized_range(unit, start, start + length)?;
                let mut piece = body.clone();
                piece.fragments[0].contribution.range = range;
                Some(piece)
            };
            let length = fit_prefix(text, preferred, &mut |length| match make(length) {
                Some(piece) => Ok(self.prepare(&piece, count)?.token_count <= MAX_TOKENS),
                None => Ok(false),
            })?;
            let mut piece = make(length).ok_or_else(structure_error)?;
            if !code && length < text.len() && !whitespace.contains(&length) {
                if let Some(&boundary) = whitespace.iter().rev().find(|&&p| p > 0 && p < length) {
                    if let Some(candidate) = make(boundary) {
                        if self.prepare(&candidate, count)?.token_count <= MAX_TOKENS {
                            piece = candidate;
                        }
                    }
                }
            }
            let end = piece.fragments[0].contribution.range.end;
            // The unit's own end is no cut: the last piece is named like the cuts before it.
            let last = end == stop;
            piece.fragments[0].split = if code {
                let starts_line = start == 0 || unit.text[..start].ends_with('\n');
                let ends_line = end == unit.text.len() || unit.text[..end].ends_with('\n');
                if starts_line && ends_line {
                    SplitKind::CodeLine
                } else {
                    SplitKind::CodeLineFragment
                }
            } else if cell {
                SplitKind::CellFragment
            } else if meaningful.contains(&(end - start)) || (last && !meaningful.is_empty()) {
                SplitKind::Sentence
            } else if whitespace.contains(&(end - start)) || (last && !whitespace.is_empty()) {
                SplitKind::Whitespace
            } else {
                SplitKind::Scalar
            };
            let prepared = self.prepare(&piece, count)?;
            if prepared.token_count > MAX_TOKENS {
                return Err(structure_error());
            }
            result.push(prepared);
            start = end;
        }
        Ok(result)
    }

    /// Whether the body carries a unit's whole text in one fragment.
    pub(super) fn full(&self, unit: usize, body: &Body) -> bool {
        body.fragments.iter().any(|f| {
            f.contribution.unit_index == unit
                && f.contribution.range
                    == TextRange {
                        start: 0,
                        end: self.mapped.units[unit].text.len(),
                    }
        })
    }

    /// The units a block owns directly, in order.
    pub(super) fn owned_units(&self, block: &str) -> Vec<usize> {
        self.mapped
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.block_id == block)
            .map(|(i, _)| i)
            .collect()
    }
}
