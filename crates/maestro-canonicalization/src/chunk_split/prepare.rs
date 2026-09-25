//! The prepared input: body parts, formatting, sentence boundaries and fitting prefixes.
use super::refusal::structure_error;
use super::structure::{Body, Layout};
use crate::{
    chunks::{
        ChunkContent, InputPart, InputRole, MappingRun, OriginMode, SourceUnit, TableWindow,
        TextRange,
    },
    content::BlockType,
    error::Error,
};
use std::collections::BTreeSet;

/// Characters after which whitespace ends a sentence: ASCII and full-width.
const SENTENCE_ENDS: [char; 6] = ['.', '!', '?', '。', '！', '？'];

impl Layout<'_> {
    /// The body's input parts: a table window's cells separated by tabs, else each fragment with
    /// its item marker and the separators between items and blocks.
    fn body_parts(&self, body: &Body) -> Result<Vec<InputPart>, Error> {
        if !body.windows.is_empty() {
            return self.window_parts(body);
        }
        let mut parts = Vec::new();
        let mut previous: Option<usize> = None;
        let mut seen_items = BTreeSet::new();
        for fragment in &body.fragments {
            let index = fragment.contribution.unit_index;
            if let Some(previous) = previous {
                self.body_separator(&mut parts, previous, index)?;
            }
            let first_of_item = self
                .nearest(index, &BlockType::ListItem)
                .is_some_and(|item| seen_items.insert(item.block_id.as_str()));
            if first_of_item {
                self.item_prefix(index, &mut parts)?;
            }
            self.fragment_parts(fragment, &mut parts)?;
            previous = Some(index);
        }
        Ok(parts)
    }

    /// A table body's input parts: its item marker, then each window's cells separated by tabs
    /// and its rows by indented line breaks.
    fn window_parts(&self, body: &Body) -> Result<Vec<InputPart>, Error> {
        let mut parts = Vec::new();
        let index = body
            .fragments
            .first()
            .ok_or_else(structure_error)?
            .contribution
            .unit_index;
        self.item_prefix(index, &mut parts)?;
        for (row, window) in body.windows.iter().enumerate() {
            if row > 0 {
                formatting(&mut parts, format!("\n{}", self.indentation(index)), true);
            }
            self.row_parts(body, window, &mut parts)?;
        }
        Ok(parts)
    }

    /// One window's cells, separated by tabs: the body's fragments in each cell.
    fn row_parts(
        &self,
        body: &Body,
        window: &TableWindow,
        parts: &mut Vec<InputPart>,
    ) -> Result<(), Error> {
        let cells = self.row_cells(&window.row_id)?;
        for (position, &column) in window.columns.iter().enumerate() {
            if position > 0 {
                formatting(parts, "\t".into(), true);
            }
            let cell = cells.get(column).ok_or_else(structure_error)?;
            for fragment in body
                .fragments
                .iter()
                .filter(|fragment| self.in_block(fragment.contribution.unit_index, &cell.block_id))
            {
                self.fragment_parts(fragment, parts)?;
            }
        }
        Ok(())
    }

    /// The separator before a body fragment: a line break between list items, a blank line
    /// between blocks of one item.
    fn body_separator(
        &self,
        parts: &mut Vec<InputPart>,
        previous: usize,
        index: usize,
    ) -> Result<(), Error> {
        let item = self.nearest(index, &BlockType::ListItem);
        let previous_item = self.nearest(previous, &BlockType::ListItem);
        if item.map(|found| &found.block_id) != previous_item.map(|found| &found.block_id) {
            formatting(parts, "\n".into(), true);
        } else if self.unit(index)?.block_id != self.unit(previous)?.block_id {
            let separator = format!("\n{}", self.indentation(index));
            formatting(parts, separator.repeat(2), true);
        }
        Ok(())
    }

    /// The chunk a body makes: its context, a blank line and its body, counted as one complete
    /// prepared input, with its section, containers and heading path.
    pub(super) fn prepare(
        &self,
        body: &Body,
        count: &mut impl FnMut(&str) -> Result<usize, Error>,
    ) -> Result<ChunkContent, Error> {
        let first = body
            .fragments
            .first()
            .ok_or_else(structure_error)?
            .contribution
            .unit_index;
        let mut parts = Vec::new();
        for entry in self.context_entries(body)? {
            let context = self.context_parts(&entry)?;
            if context.is_empty() {
                continue;
            }
            if !parts.is_empty() {
                formatting(&mut parts, "\n".into(), false);
            }
            parts.extend(context);
        }
        if !parts.is_empty() {
            formatting(&mut parts, "\n\n".into(), false);
        }
        parts.extend(self.body_parts(body)?);
        let mut prepared_input = String::new();
        let mut body_text = String::new();
        for part in &mut parts {
            let start = prepared_input.len();
            prepared_input.push_str(&part.text);
            part.prepared_range = TextRange {
                start,
                end: prepared_input.len(),
            };
            if part.role == InputRole::SourceContent || part.body_layout {
                body_text.push_str(&part.text);
            }
        }
        if body_text.is_empty() {
            return Err(structure_error());
        }
        let token_count = count(&prepared_input)?;
        let mut container_ids = Vec::new();
        let ancestors = body
            .fragments
            .iter()
            .flat_map(|fragment| self.ancestors(fragment.contribution.unit_index));
        for block in ancestors {
            if !container_ids.contains(&block.block_id) {
                container_ids.push(block.block_id.clone());
            }
        }
        Ok(ChunkContent {
            section_id: self.section(first),
            container_ids,
            heading_path: self.owner(first)?.heading_path.clone(),
            fragments: body.fragments.clone(),
            table_windows: body.windows.clone(),
            body_text,
            input_parts: parts,
            prepared_input,
            token_count,
        })
    }
}

/// Append a formatting separator: text that no source contributed, mapped as formatting only; empty
/// text adds nothing.
pub(super) fn formatting(parts: &mut Vec<InputPart>, text: String, body_layout: bool) {
    if text.is_empty() {
        return;
    }
    let range = TextRange {
        start: 0,
        end: text.len(),
    };
    parts.push(InputPart {
        role: InputRole::FormattingSeparator,
        text,
        prepared_range: range,
        contributions: Vec::new(),
        mappings: vec![MappingRun {
            range,
            mode: OriginMode::Formatting,
            origins: Vec::new(),
        }],
        body_layout,
    });
}

/// A range of a unit's text that neither starts nor ends inside an inline delimiter, extended over
/// closing delimiters and holding some text outside them; none otherwise.
pub(super) fn normalized_range(
    unit: &SourceUnit,
    start: usize,
    mut end: usize,
) -> Option<TextRange> {
    unit.text.get(start..end)?;
    if start == end {
        return None;
    }
    for envelope in &unit.envelopes {
        for wrapper in [envelope.opening, envelope.closing] {
            if wrapper.start < start && start < wrapper.end {
                return None;
            }
        }
        if envelope.opening.start < end && end <= envelope.opening.end {
            return None;
        }
    }
    // Take in closing delimiters; each pass moves past one for good: one pass per envelope.
    for _ in &unit.envelopes {
        let next = unit
            .envelopes
            .iter()
            .filter(|envelope| envelope.closing.start <= end && end < envelope.closing.end)
            .map(|envelope| envelope.closing.end)
            .max();
        match next {
            Some(next) => end = next,
            None => break,
        }
    }
    let text = unit.text.get(start..end)?;
    let substantive = text.char_indices().any(|(offset, _)| {
        let at = start + offset;
        !unit.envelopes.iter().any(|envelope| {
            [envelope.opening, envelope.closing]
                .iter()
                .any(|wrapper| wrapper.start <= at && at < wrapper.end)
        })
    });
    substantive.then_some(TextRange { start, end })
}

/// Candidate cut points after whitespace: the meaningful ones follow a line break in code or a
/// sentence end elsewhere; the others follow any whitespace.
pub(super) fn boundaries(text: &str, code: bool) -> (Vec<usize>, Vec<usize>) {
    let mut meaningful = Vec::new();
    let mut whitespace = Vec::new();
    let mut previous = None;
    for (position, character) in text.char_indices() {
        let end = position + character.len_utf8();
        if character.is_whitespace() {
            whitespace.push(end);
            if (code && character == '\n')
                || (!code && previous.is_some_and(|before| SENTENCE_ENDS.contains(&before)))
            {
                meaningful.push(end);
            }
        }
        previous = Some(character);
    }
    (meaningful, whitespace)
}

/// A prefix of the text that fits: the whole text, or the first fitting half by halving, then cut
/// back to the last preferred boundary if that also fits; a single character that does not fit is
/// refused.
pub(super) fn fit_prefix(
    text: &str,
    preferred: &[usize],
    fits: &mut impl FnMut(usize) -> Result<bool, Error>,
) -> Result<usize, Error> {
    let mut end = text.len();
    loop {
        if end > 0 && fits(end)? {
            // A rest that fits stays whole; only a halved prefix moves back to a preferred cut.
            if end == text.len() {
                return Ok(end);
            }
            let boundary = preferred
                .iter()
                .rev()
                .copied()
                .find(|&cut| cut > 0 && cut < end && text.is_char_boundary(cut));
            return match boundary {
                Some(boundary) if fits(boundary)? => Ok(boundary),
                Some(_) | None => Ok(end),
            };
        }
        let scalar_count = text[..end].chars().count();
        if scalar_count <= 1 {
            return Err(structure_error());
        }
        end = text
            .char_indices()
            .nth(scalar_count / 2)
            .map(|(offset, _)| offset)
            .ok_or_else(structure_error)?;
    }
}
