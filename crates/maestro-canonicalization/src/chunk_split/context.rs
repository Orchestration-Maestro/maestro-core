//! The context a chunk repeats: headings, parent items, task markers and table headers.
use super::prepare::{formatting, normalized_range};
use super::refusal::structure_error;
use super::structure::{Body, ContextEntry, Layout};
use crate::{
    chunk_mapping::mapped_slice,
    content::{Block, BlockType, ContentNode, InlineKind},
    error::Error,
    prepared_inputs::{Contribution, Fragment, InputPart, InputRole},
    source_units::{TextRange, UnitField},
};
use std::collections::BTreeSet;

impl Layout<'_> {
    /// The context a body repeats: its enclosing headings, its ancestors' context and a table
    /// header, less what the body holds whole, headings first.
    pub(super) fn context_entries(&self, body: &Body) -> Result<Vec<ContextEntry>, Error> {
        let first = body
            .fragments
            .first()
            .ok_or_else(structure_error)?
            .contribution
            .unit_index;
        let headings = self.heading_chain(first)?;
        let mut entries = self.heading_entries(body, &headings);
        entries.extend(self.ancestor_entries(body)?);
        entries.extend(self.table_header_entry(body, first)?);
        entries.retain(|entry| {
            let primary: Vec<_> = entry
                .units
                .iter()
                .copied()
                .filter(|&unit| self.is_primary(unit))
                .collect();
            primary.is_empty() || !primary.iter().all(|&unit| self.full(unit, body))
        });
        entries.sort_by_key(|entry| (entry.role != InputRole::HeadingContext, entry.depth));
        Ok(entries)
    }

    /// The sections above a unit, outermost first; a cycle or a non-heading
    /// section is a structure error.
    fn heading_chain(&self, first: usize) -> Result<Vec<String>, Error> {
        let mut headings = Vec::new();
        let mut section = self.section(first);
        while let Some(id) = section.take() {
            let heading = self
                .blocks
                .get(id.as_str())
                .copied()
                .ok_or_else(structure_error)?;
            if heading.block_type != BlockType::Heading || headings.contains(&id) {
                return Err(structure_error());
            }
            headings.push(id);
            section.clone_from(&heading.parent_section_id);
        }
        headings.reverse();
        Ok(headings)
    }

    /// Heading context for each enclosing heading the body does not consist of.
    fn heading_entries(&self, body: &Body, headings: &[String]) -> Vec<ContextEntry> {
        let mut entries = Vec::new();
        for (depth, id) in headings.iter().enumerate() {
            // A heading's own primary text is not its unsplit continuation context.
            if body
                .fragments
                .iter()
                .all(|fragment| self.in_block(fragment.contribution.unit_index, id))
            {
                continue;
            }
            let units = self
                .owned_units(id)
                .into_iter()
                .filter(|&unit| self.is_primary(unit))
                .collect();
            entries.push(ContextEntry {
                role: InputRole::HeadingContext,
                depth,
                units,
                columns: None,
            });
        }
        entries
    }

    /// Context from every ancestor block of the body's fragments, each once.
    fn ancestor_entries(&self, body: &Body) -> Result<Vec<ContextEntry>, Error> {
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for fragment in &body.fragments {
            let index = fragment.contribution.unit_index;
            let own_item = self
                .nearest(index, &BlockType::ListItem)
                .map(|item| item.block_id.as_str());
            for (depth, block) in self.ancestors(index).iter().enumerate() {
                entries.extend(self.ancestor_block_entries(block, depth, own_item, &mut seen)?);
            }
        }
        Ok(entries)
    }

    /// One ancestor's context: a parent item, the own item's task marker,
    /// non-primary attributes and a definition's term; `seen` keeps each once.
    fn ancestor_block_entries(
        &self,
        block: &Block,
        depth: usize,
        own_item: Option<&str>,
        seen: &mut BTreeSet<String>,
    ) -> Result<Vec<ContextEntry>, Error> {
        let mut entries = Vec::new();
        if block.block_type == BlockType::ListItem
            && Some(block.block_id.as_str()) != own_item
            && seen.insert(format!("parent:{}", block.block_id))
        {
            entries.push(ContextEntry {
                role: InputRole::ParentListContext,
                depth,
                units: self.list_item_units(&block.block_id),
                columns: None,
            });
        }
        if block.block_type == BlockType::ListItem
            && Some(block.block_id.as_str()) == own_item
            && seen.insert(format!("task:{}", block.block_id))
        {
            let units = self.task_marker_units(&block.block_id)?;
            if !units.is_empty() {
                entries.push(ContextEntry {
                    role: InputRole::StructuralContext,
                    depth,
                    units,
                    columns: None,
                });
            }
        }
        for unit in self
            .owned_units(&block.block_id)
            .into_iter()
            .filter(|&unit| {
                self.mapped
                    .units
                    .get(unit)
                    .is_some_and(|owned| !owned.primary && owned.field != UnitField::ListMarker)
            })
        {
            if seen.insert(format!("attribute:{unit}")) {
                entries.push(ContextEntry {
                    role: InputRole::StructuralContext,
                    depth,
                    units: vec![unit],
                    columns: None,
                });
            }
        }
        if block.block_type == BlockType::DefinitionDescription {
            let term = self.definition_term(block)?;
            if seen.insert(format!("term:{}", term.block_id)) {
                entries.push(ContextEntry {
                    role: InputRole::StructuralContext,
                    depth,
                    units: self.owned_units(&term.block_id),
                    columns: None,
                });
            }
        }
        Ok(entries)
    }

    /// Every unit whose nearest list item is `item_id`.
    fn list_item_units(&self, item_id: &str) -> Vec<usize> {
        self.mapped
            .units
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                self.nearest(*index, &BlockType::ListItem)
                    .is_some_and(|item| item.block_id == item_id)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// The units of `item_id`'s own task marker, if it has one.
    fn task_marker_units(&self, item_id: &str) -> Result<Vec<usize>, Error> {
        let mut units = Vec::new();
        for (index, unit) in self.mapped.units.iter().enumerate() {
            if self
                .nearest(index, &BlockType::ListItem)
                .is_none_or(|item| item.block_id != item_id)
            {
                continue;
            }
            let UnitField::Inline { child_path } = &unit.field else {
                continue;
            };
            let root = *child_path.first().ok_or_else(structure_error)?;
            let child = self.owner(index)?.structured_content.children.get(root);
            if Self::is_task_marker(child) {
                units.push(index);
            }
        }
        Ok(units)
    }

    /// The last definition term before a definition description.
    fn definition_term(&self, description: &Block) -> Result<&Block, Error> {
        let parent = self
            .blocks
            .get(
                description
                    .parent_block_id
                    .as_deref()
                    .ok_or_else(structure_error)?,
            )
            .copied()
            .ok_or_else(structure_error)?;
        self.child_blocks(parent)
            .into_iter()
            .take_while(|block| block.block_id != description.block_id)
            .filter(|block| block.block_type == BlockType::DefinitionTerm)
            .last()
            .ok_or_else(structure_error)
    }

    /// The header context for a table row other than the first: the header
    /// cells of the window's columns.
    fn table_header_entry(&self, body: &Body, first: usize) -> Result<Option<ContextEntry>, Error> {
        let Some(window) = body
            .windows
            .first()
            .filter(|_| body.windows.iter().any(|window| window.row_index != 0))
        else {
            return Ok(None);
        };
        let table = self
            .blocks
            .get(window.table_id.as_str())
            .copied()
            .ok_or_else(structure_error)?;
        let header = self
            .child_blocks(table)
            .into_iter()
            .find(|block| block.block_type == BlockType::TableHead)
            .ok_or_else(structure_error)?;
        let cells = self.row_cells(&header.block_id)?;
        let columns: Vec<Vec<usize>> = window
            .columns
            .iter()
            .map(|&column| {
                cells
                    .get(column)
                    .map(|cell| self.owned_units(&cell.block_id))
                    .ok_or_else(structure_error)
            })
            .collect::<Result<_, _>>()?;
        Ok(Some(ContextEntry {
            role: InputRole::TableHeaderContext,
            depth: self.ancestors(first).len(),
            units: columns.iter().flatten().copied().collect(),
            columns: Some(columns),
        }))
    }

    /// Whether a structured-content node is a task list item's checkbox marker.
    fn is_task_marker(node: Option<&ContentNode>) -> bool {
        matches!(
            node,
            Some(ContentNode::Inline { inline })
                if matches!(inline.content, InlineKind::TaskMarker { .. })
        )
    }

    /// An input part for a contribution: its text and source mappings, in the given role.
    fn part(&self, contribution: Contribution, role: InputRole) -> Result<InputPart, Error> {
        let unit = self
            .mapped
            .units
            .get(contribution.unit_index)
            .ok_or_else(structure_error)?;
        let text = unit
            .text
            .get(contribution.range.start..contribution.range.end)
            .ok_or_else(structure_error)?
            .to_owned();
        Ok(InputPart {
            role,
            text,
            prepared_range: TextRange { start: 0, end: 0 },
            mappings: mapped_slice(unit, contribution.range, self.markdown)?,
            contributions: vec![contribution],
            body_layout: false,
        })
    }

    /// An input part for a unit's whole text.
    fn whole_part(&self, index: usize, role: InputRole) -> Result<InputPart, Error> {
        self.part(
            Contribution {
                unit_index: index,
                range: TextRange {
                    start: 0,
                    end: self.unit(index)?.text.len(),
                },
            },
            role,
        )
    }

    /// Two spaces for each list around a unit: the indentation its continuation lines carry.
    pub(super) fn indentation(&self, index: usize) -> String {
        "  ".repeat(self.list_depth(index))
    }

    /// How many lists a unit is inside.
    fn list_depth(&self, index: usize) -> usize {
        self.ancestors(index)
            .iter()
            .filter(|block| block.block_type == BlockType::List)
            .count()
    }

    /// Append a contribution line by line, each line break followed by the unit's list indentation
    /// as formatting.
    fn append_source(
        &self,
        parts: &mut Vec<InputPart>,
        contribution: Contribution,
        role: InputRole,
    ) -> Result<(), Error> {
        let unit = self.unit(contribution.unit_index)?;
        let text = unit
            .text
            .get(contribution.range.start..contribution.range.end)
            .ok_or_else(structure_error)?;
        let indent = self.indentation(contribution.unit_index);
        let mut start = contribution.range.start;
        for line in text.split_inclusive('\n') {
            let end = start + line.len();
            parts.push(self.part(
                Contribution {
                    range: TextRange { start, end },
                    ..contribution
                },
                role,
            )?);
            if line.ends_with('\n') {
                formatting(parts, indent.clone(), role == InputRole::SourceContent);
            }
            start = end;
        }
        Ok(())
    }

    /// The input parts of a context entry: a table header's columns separated by tabs, else its
    /// units with the separators their blocks need.
    pub(super) fn context_parts(&self, entry: &ContextEntry) -> Result<Vec<InputPart>, Error> {
        if let Some(columns) = &entry.columns {
            return self.header_parts(columns, entry.role);
        }
        let mut result = Vec::new();
        let mut previous: Option<usize> = None;
        for &index in &entry.units {
            let unit = self.unit(index)?;
            if let Some(previous) = previous {
                self.context_separator(&mut result, previous, index)?;
            } else if entry.role == InputRole::ParentListContext {
                let depth = self.list_depth(index);
                formatting(&mut result, "  ".repeat(depth.saturating_sub(1)), false);
            }
            self.append_source(
                &mut result,
                Contribution {
                    unit_index: index,
                    range: TextRange {
                        start: 0,
                        end: unit.text.len(),
                    },
                },
                entry.role,
            )?;
            previous = Some(index);
        }
        Ok(result)
    }

    /// A table header's input parts: each column's units, the columns separated by tabs.
    fn header_parts(
        &self,
        columns: &[Vec<usize>],
        role: InputRole,
    ) -> Result<Vec<InputPart>, Error> {
        let mut result = Vec::new();
        for (column, units) in columns.iter().enumerate() {
            if column > 0 {
                formatting(&mut result, "\t".into(), false);
            }
            for &index in units {
                result.push(self.whole_part(index, role)?);
            }
        }
        Ok(result)
    }

    /// The separator between two context units: none after a list marker, a blank line between
    /// blocks, a line break after a block's non-primary attribute.
    fn context_separator(
        &self,
        result: &mut Vec<InputPart>,
        previous: usize,
        index: usize,
    ) -> Result<(), Error> {
        let preceding = self.unit(previous)?;
        if preceding.field == UnitField::ListMarker {
            return Ok(());
        }
        if preceding.block_id != self.unit(index)?.block_id {
            let separator = format!("\n{}", self.indentation(index));
            formatting(result, separator.repeat(2), false);
        } else if !preceding.primary {
            formatting(result, format!("\n{}", self.indentation(index)), false);
        }
        Ok(())
    }

    /// Append a body fragment, repeating the delimiters of the inline envelopes, such as deletions,
    /// that it starts or ends inside.
    pub(super) fn fragment_parts(
        &self,
        fragment: &Fragment,
        parts: &mut Vec<InputPart>,
    ) -> Result<(), Error> {
        let contribution = fragment.contribution;
        let unit = self.unit(contribution.unit_index)?;
        let range = contribution.range;
        if normalized_range(unit, range.start, range.end) != Some(range) {
            return Err(structure_error());
        }
        let active: Vec<_> = unit
            .envelopes
            .iter()
            .filter(|envelope| {
                range.start < envelope.closing.start && range.end > envelope.opening.end
            })
            .collect();
        for envelope in &active {
            if range.start > envelope.opening.start {
                parts.push(self.part(
                    Contribution {
                        range: envelope.opening,
                        ..contribution
                    },
                    InputRole::StructuralContext,
                )?);
            }
        }
        self.append_source(parts, contribution, InputRole::SourceContent)?;
        for envelope in active.into_iter().rev() {
            if range.end < envelope.closing.end {
                parts.push(self.part(
                    Contribution {
                        range: envelope.closing,
                        ..contribution
                    },
                    InputRole::StructuralContext,
                )?);
            }
        }
        Ok(())
    }

    /// Append the indentation and the source-backed marker of the list item a unit is in, if any.
    pub(super) fn item_prefix(
        &self,
        index: usize,
        parts: &mut Vec<InputPart>,
    ) -> Result<(), Error> {
        if let Some(item) = self.nearest(index, &BlockType::ListItem) {
            let depth = self.list_depth(index);
            formatting(parts, "  ".repeat(depth.saturating_sub(1)), true);
            let marker = self
                .owned_units(&item.block_id)
                .into_iter()
                .find(|&unit| {
                    self.mapped
                        .units
                        .get(unit)
                        .is_some_and(|owned| owned.field == UnitField::ListMarker)
                })
                .ok_or_else(structure_error)?;
            // Canonical markers/ordinals are source-backed context, unlike indentation.
            parts.push(self.whole_part(marker, InputRole::StructuralContext)?);
        }
        Ok(())
    }
}
