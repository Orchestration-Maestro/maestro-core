//! Replay checks: coverage and every prepared part must rebuild from the mapped source.
use super::{
    ChunkContent, Contribution, InputPart, InputRole, MappedDocument, MappingRun, OriginMode,
    TextRange, UnitCoverage, invalid_chunks,
};
use crate::{
    CanonicalDocument, Error,
    chunk_mapping::{map_accounting, mapped_slice},
    chunk_split::{MAX_TOKENS, validate_preparation},
};

/// Check that the chunks' fragments cover each primary unit's text exactly once and in order;
/// returns each unit's ranges.
pub(super) fn validate_coverage(
    mapped: &MappedDocument,
    chunks: &[ChunkContent],
) -> Result<Vec<UnitCoverage>, Error> {
    let mut ranges = vec![Vec::new(); mapped.units.len()];
    let mut previous_unit = None;
    for fragment in chunks.iter().flat_map(|chunk| &chunk.fragments) {
        let Contribution { unit_index, range } = fragment.contribution;
        let unit = mapped.units.get(unit_index).ok_or_else(invalid_chunks)?;
        let selected = ranges.get_mut(unit_index).ok_or_else(invalid_chunks)?;
        if !unit.primary
            || range.start >= range.end
            || unit.text.get(range.start..range.end).is_none()
            || fragment.part_ordinal != selected.len()
            || previous_unit.is_some_and(|previous| previous > unit_index)
        {
            return Err(invalid_chunks());
        }
        previous_unit = Some(unit_index);
        selected.push(range);
    }
    let mut coverage = Vec::new();
    for (unit_index, (unit, mut ranges)) in mapped.units.iter().zip(ranges).enumerate() {
        if !unit.primary {
            continue;
        }
        ranges.sort_unstable_by_key(|range| (range.start, range.end));
        let mut next = 0;
        for range in &ranges {
            if range.start != next {
                return Err(invalid_chunks());
            }
            next = range.end;
        }
        if next != unit.text.len() {
            return Err(invalid_chunks());
        }
        coverage.push(UnitCoverage {
            unit_index,
            primary_ranges: ranges,
        });
    }
    Ok(coverage)
}

/// Rebuild every chunk from the mapped source and recount it: its parts, mappings, body text,
/// fragment order and preparation must all match.
pub(super) fn validate_chunks(
    document: &CanonicalDocument,
    markdown: &str,
    mapped: &MappedDocument,
    chunks: &[ChunkContent],
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<(), Error> {
    if mapped.accounting != map_accounting(document, markdown, &mapped.units)? {
        return Err(invalid_chunks());
    }
    for chunk in chunks {
        if chunk.fragments.is_empty()
            || chunk.token_count > MAX_TOKENS
            || count(&chunk.prepared_input)? != chunk.token_count
        {
            return Err(invalid_chunks());
        }
        let mut prepared = String::new();
        let mut body = String::new();
        let mut primary = Vec::new();
        for part in &chunk.input_parts {
            let start = prepared.len();
            prepared.push_str(&part.text);
            if part.text.is_empty()
                || part.prepared_range
                    != (TextRange {
                        start,
                        end: prepared.len(),
                    })
            {
                return Err(invalid_chunks());
            }
            if part.role == InputRole::SourceContent || part.body_layout {
                body.push_str(&part.text);
            }
            if part.role == InputRole::FormattingSeparator {
                check_separator(part)?;
                continue;
            }
            primary.extend(replay_part(part, mapped, markdown)?);
        }
        if prepared != chunk.prepared_input || body != chunk.body_text || body.is_empty() {
            return Err(invalid_chunks());
        }
        check_fragment_order(chunk, &primary)?;
    }
    validate_preparation(document, markdown, mapped, chunks, count)
}

/// A formatting separator contributes no source and maps as formatting only.
fn check_separator(part: &InputPart) -> Result<(), Error> {
    if !part.contributions.is_empty()
        || part.mappings
            != vec![MappingRun {
                range: TextRange {
                    start: 0,
                    end: part.text.len(),
                },
                mode: OriginMode::Formatting,
                origins: Vec::new(),
            }]
    {
        return Err(invalid_chunks());
    }
    Ok(())
}

/// Rebuild a source part's text and mappings from its contributions; returns
/// the primary contributions of a source-content part.
fn replay_part(
    part: &InputPart,
    mapped: &MappedDocument,
    markdown: &str,
) -> Result<Vec<Contribution>, Error> {
    if part.body_layout || part.contributions.is_empty() {
        return Err(invalid_chunks());
    }
    let mut primary = Vec::new();
    let mut text = String::new();
    let mut mappings = Vec::new();
    for contribution in &part.contributions {
        let unit = mapped
            .units
            .get(contribution.unit_index)
            .ok_or_else(invalid_chunks)?;
        let selected = unit
            .text
            .get(contribution.range.start..contribution.range.end)
            .ok_or_else(invalid_chunks)?;
        if selected.is_empty() {
            return Err(invalid_chunks());
        }
        let mut runs = mapped_slice(unit, contribution.range, markdown)?;
        for run in &mut runs {
            run.range.start += text.len();
            run.range.end += text.len();
        }
        mappings.extend(runs);
        text.push_str(selected);
        if part.role == InputRole::SourceContent {
            if !unit.primary {
                return Err(invalid_chunks());
            }
            primary.push(*contribution);
        }
    }
    if text != part.text || mappings != part.mappings {
        return Err(invalid_chunks());
    }
    Ok(primary)
}

/// The primary contributions walk the chunk's fragments in order, each
/// fragment covered exactly, with no gap and no overlap.
fn check_fragment_order(chunk: &ChunkContent, primary: &[Contribution]) -> Result<(), Error> {
    let mut fragment = 0;
    let mut cursor = chunk.fragments[0].contribution.range.start;
    for contribution in primary {
        let expected = chunk
            .fragments
            .get(fragment)
            .ok_or_else(invalid_chunks)?
            .contribution;
        if contribution.unit_index != expected.unit_index
            || contribution.range.start != cursor
            || contribution.range.end > expected.range.end
        {
            return Err(invalid_chunks());
        }
        cursor = contribution.range.end;
        if cursor == expected.range.end {
            fragment += 1;
            cursor = chunk
                .fragments
                .get(fragment)
                .map_or(0, |next_fragment| next_fragment.contribution.range.start);
        }
    }
    if fragment != chunk.fragments.len() {
        return Err(invalid_chunks());
    }
    Ok(())
}
