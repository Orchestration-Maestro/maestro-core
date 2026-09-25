//! The layout's structural queries: owners, sections, table windows and packing atoms.
use super::limits::MAX_TOKENS;
use super::prepare::{boundaries, fit_prefix, normalized_range};
use super::refusal::structure_error;
use super::structure::{Body, Layout};
use crate::{
    Error,
    chunks::{ChunkContent, Contribution, Fragment, SplitKind, TableWindow, TextRange},
    content::{Block, BlockType, ContentNode},
};

impl Layout<'_> {
    /// The block that owns a unit, the last of its ancestry; an unknown unit is a structure error.
    pub(super) fn owner(&self, unit: usize) -> Result<&Block, Error> {
        self.ancestors(unit)
            .last()
            .copied()
            .ok_or_else(structure_error)
    }

    /// The innermost ancestor of a unit with the given block type.
    pub(super) fn nearest(&self, unit: usize, kind: &BlockType) -> Option<&Block> {
        self.ancestors(unit)
            .iter()
            .rev()
            .copied()
            .find(|block| block.block_type == *kind)
    }

    /// The heading whose section holds a unit: the unit's own heading, or its owner's section.
    pub(super) fn section(&self, unit: usize) -> Option<String> {
        let owner = self.ancestors(unit).last()?;
        if owner.block_type == BlockType::Heading {
            Some(owner.block_id.clone())
        } else {
            owner.parent_section_id.clone()
        }
    }

    /// What two units must share to share a chunk: their section and their enclosing containers
    /// (lists, code, tables, quotes, notes, definition lists, HTML and raw source).
    fn key(&self, unit: usize) -> (Option<String>, Vec<String>) {
        let containers = self
            .ancestors(unit)
            .iter()
            .filter(|block| {
                matches!(
                    block.block_type,
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
            .map(|block| block.block_id.clone())
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
            .filter(|block| block.block_type == BlockType::TableCell)
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
        let row = self
            .ancestors(unit)
            .iter()
            .rev()
            .copied()
            .find(|block| matches!(block.block_type, BlockType::TableHead | BlockType::TableRow))
            .ok_or_else(structure_error)?;
        let cell = self
            .nearest(unit, &BlockType::TableCell)
            .ok_or_else(structure_error)?;
        let rows: Vec<_> = self
            .child_blocks(table)
            .into_iter()
            .filter(|block| matches!(block.block_type, BlockType::TableHead | BlockType::TableRow))
            .collect();
        let cells = self.row_cells(&row.block_id)?;
        let column = cells
            .iter()
            .position(|block| block.block_id == cell.block_id)
            .ok_or_else(structure_error)?;
        Ok(Some(TableWindow {
            table_id: table.block_id.clone(),
            row_id: row.block_id.clone(),
            row_index: rows
                .iter()
                .position(|block| block.block_id == row.block_id)
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
            .filter(|(_, unit)| unit.primary)
        {
            let window = self.window(index, true)?;
            let natural = window.as_ref().map_or_else(
                || {
                    self.enclosing_item(index)
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

    /// The innermost list item or definition description around a unit.
    fn enclosing_item(&self, unit: usize) -> Option<&Block> {
        self.ancestors(unit).iter().rev().copied().find(|block| {
            matches!(
                block.block_type,
                BlockType::ListItem | BlockType::DefinitionDescription
            )
        })
    }

    /// Whether two bodies may share a chunk: the same key, and either no table or one table with
    /// the same row or the same columns.
    pub(super) fn compatible(&self, left: &Body, right: &Body) -> bool {
        let (Some(left_first), Some(right_first)) =
            (left.fragments.first(), right.fragments.first())
        else {
            return false;
        };
        if self.key(left_first.contribution.unit_index)
            != self.key(right_first.contribution.unit_index)
        {
            return false;
        }
        match (left.windows.last(), right.windows.first()) {
            (Some(last), Some(first)) => {
                last.table_id == first.table_id
                    && (last.row_id == first.row_id || last.columns == first.columns)
            }
            (None, None) => true,
            _ => false,
        }
    }

    /// Split an oversized body at its structure: a multi-column row into one body per column, else
    /// each fragment into its own body; nothing for a single fragment.
    pub(super) fn refine(&self, body: &Body) -> Result<Option<Vec<Body>>, Error> {
        if let Some(window) = body
            .windows
            .first()
            .filter(|window| window.columns.len() > 1)
        {
            let result = self.column_bodies(body, window)?;
            if !result.is_empty() {
                return Ok(Some(result));
            }
        }
        if body.fragments.len() > 1 {
            let mut result = Vec::new();
            for fragment in &body.fragments {
                result.push(self.fragment_body(fragment)?);
            }
            return Ok(Some(result));
        }
        Ok(None)
    }

    /// One body for each column of a multi-column window that holds fragments of the body, each
    /// fragment split at the structure.
    fn column_bodies(&self, body: &Body, window: &TableWindow) -> Result<Vec<Body>, Error> {
        let cells = self.row_cells(&window.row_id)?;
        let mut result = Vec::new();
        for &column in &window.columns {
            let cell = cells.get(column).ok_or_else(structure_error)?;
            let fragments: Vec<_> = body
                .fragments
                .iter()
                .filter(|fragment| self.in_block(fragment.contribution.unit_index, &cell.block_id))
                .cloned()
                .map(|mut fragment| {
                    fragment.split = SplitKind::Structural;
                    fragment
                })
                .collect();
            if fragments.is_empty() {
                continue;
            }
            let mut selected = window.clone();
            selected.columns = vec![column];
            result.push(Body {
                fragments,
                windows: vec![selected],
            });
        }
        Ok(result)
    }

    /// A fragment as a body of its own with its own table window, split at a code line or else at
    /// the structure.
    fn fragment_body(&self, fragment: &Fragment) -> Result<Body, Error> {
        let mut fragment = fragment.clone();
        fragment.split = if self
            .nearest(fragment.contribution.unit_index, &BlockType::Code)
            .is_some()
        {
            SplitKind::CodeLine
        } else {
            SplitKind::Structural
        };
        Ok(Body {
            windows: self
                .window(fragment.contribution.unit_index, false)?
                .into_iter()
                .collect(),
            fragments: vec![fragment],
        })
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
        let unit = self.unit(index)?;
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
                piece.fragments.first_mut()?.contribution.range = range;
                Some(piece)
            };
            let length = fit_prefix(text, preferred, &mut |length| match make(length) {
                Some(piece) => Ok(self.prepare(&piece, count)?.token_count <= MAX_TOKENS),
                None => Ok(false),
            })?;
            let mut piece = make(length).ok_or_else(structure_error)?;
            // A cut inside a word moves back to the last whitespace when that piece fits too.
            let retreat = !code && length < text.len() && !whitespace.contains(&length);
            let shorter = whitespace
                .iter()
                .rev()
                .copied()
                .find(|&cut| cut > 0 && cut < length)
                .filter(|_| retreat)
                .and_then(&make);
            match shorter {
                Some(candidate) if self.prepare(&candidate, count)?.token_count <= MAX_TOKENS => {
                    piece = candidate;
                }
                Some(_) | None => {}
            }
            let fragment = piece.fragments.first_mut().ok_or_else(structure_error)?;
            let end = fragment.contribution.range.end;
            // The unit's own end is no cut: the last piece is named like the cuts before it.
            let last = end == stop;
            fragment.split = if code {
                code_split(&unit.text, start, end)
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
        body.fragments.iter().any(|fragment| {
            fragment.contribution.unit_index == unit
                && self.mapped.units.get(unit).is_some_and(|whole| {
                    fragment.contribution.range
                        == TextRange {
                            start: 0,
                            end: whole.text.len(),
                        }
                })
        })
    }

    /// The units a block owns directly, in order.
    pub(super) fn owned_units(&self, block: &str) -> Vec<usize> {
        self.mapped
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| unit.block_id == block)
            .map(|(index, _)| index)
            .collect()
    }
}

/// How a piece of a code unit was cut: on line boundaries at both ends, or inside a line.
fn code_split(text: &str, start: usize, end: usize) -> SplitKind {
    let starts_line = start == 0 || text[..start].ends_with('\n');
    let ends_line = end == text.len() || text[..end].ends_with('\n');
    if starts_line && ends_line {
        SplitKind::CodeLine
    } else {
        SplitKind::CodeLineFragment
    }
}
