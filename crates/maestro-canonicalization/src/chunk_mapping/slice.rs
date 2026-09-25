//! Mapped slices and the source accounting ledger: which original bytes each unit covers.
use super::refusal::invalid_mapping;
use crate::{
    content::BlockType,
    document::{CanonicalDocument, SourceRole},
    error::Error,
    model::SourceSpan,
    source_units::{
        AccountingDisposition, MappingRun, OriginMode, SourceDisposition, SourceUnit, TextRange,
    },
};
use std::collections::{BTreeMap, BTreeSet};

/// The mapping runs of a byte range of a unit's text, with offsets relative to the range: exact
/// runs narrow to the range, transformed runs keep their whole original syntax.
pub(crate) fn mapped_slice(
    unit: &SourceUnit,
    range: TextRange,
    markdown: &str,
) -> Result<Vec<MappingRun>, Error> {
    if unit.text.get(range.start..range.end).is_none() {
        return Err(invalid_mapping());
    }
    let mut cursor = 0;
    let mut result = Vec::new();
    for run in &unit.mappings {
        let text = unit
            .text
            .get(run.range.start..run.range.end)
            .ok_or_else(invalid_mapping)?;
        if run.range.start != cursor
            || text.is_empty()
            || run
                .origins
                .iter()
                .any(|origin| !origin.span.is_valid(markdown))
        {
            return Err(invalid_mapping());
        }
        cursor = run.range.end;
        match run.mode {
            OriginMode::ExactCopy => match run.origins.as_slice() {
                [origin] if markdown.get(origin.span.start..origin.span.end) == Some(text) => {}
                _ => return Err(invalid_mapping()),
            },
            OriginMode::CanonicalTransformation if run.origins.is_empty() => {
                return Err(invalid_mapping());
            }
            OriginMode::Formatting if !run.origins.is_empty() => return Err(invalid_mapping()),
            OriginMode::CanonicalTransformation | OriginMode::Formatting => {}
        }
        let start = run.range.start.max(range.start);
        let end = run.range.end.min(range.end);
        if start >= end {
            continue;
        }
        let mut origins = run.origins.clone();
        if let (OriginMode::ExactCopy, [origin]) = (run.mode, origins.as_mut_slice()) {
            origin.span.start += start - run.range.start;
            origin.span.end = origin.span.start + (end - start);
        }
        result.push(MappingRun {
            range: TextRange {
                start: start - range.start,
                end: end - range.start,
            },
            mode: run.mode,
            origins,
        });
    }
    if cursor != unit.text.len() {
        return Err(invalid_mapping());
    }
    Ok(result)
}

/// The source ledger as the mapping sees it: each entry's disposition and the units that carry its
/// bytes.
pub(crate) fn map_accounting(
    document: &CanonicalDocument,
    markdown: &str,
    units: &[SourceUnit],
) -> Result<Vec<AccountingDisposition>, Error> {
    let mut by_owner: BTreeMap<&str, Vec<(usize, SourceSpan)>> = BTreeMap::new();
    for (index, unit) in units.iter().enumerate() {
        for origin in unit.mappings.iter().flat_map(|run| &run.origins) {
            by_owner
                .entry(&origin.block_id)
                .or_default()
                .push((index, origin.span));
        }
    }
    let blocks: BTreeMap<_, _> = document
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block))
        .collect();
    let mut cursor = 0;
    let mut result = Vec::new();
    for (accounting_index, entry) in document.source_accounting.iter().enumerate() {
        let span = entry.source_span;
        if !span.is_valid(markdown)
            || span.start != cursor
            || span.start == span.end
            || entry.role == SourceRole::Unaccounted
        {
            return Err(invalid_mapping());
        }
        cursor = span.end;
        let owner = entry
            .block_id
            .as_deref()
            .map(|id| blocks.get(id).copied().ok_or_else(invalid_mapping))
            .transpose()?;
        let disposition = match owner.map(|block| &block.block_type) {
            Some(BlockType::Metadata) => SourceDisposition::Metadata,
            Some(BlockType::ReferenceDefinition) => SourceDisposition::ReferenceDefinition,
            _ => match entry.role {
                SourceRole::ParsedContent | SourceRole::Unsupported => SourceDisposition::Eligible,
                SourceRole::StructuralSyntax => SourceDisposition::Structural,
                SourceRole::MetadataOrReference => SourceDisposition::Metadata,
                SourceRole::Unaccounted => return Err(invalid_mapping()),
            },
        };
        let matches: Vec<_> = entry
            .block_id
            .as_deref()
            .and_then(|id| by_owner.get(id))
            .into_iter()
            .flatten()
            .filter(|(_, origin)| origin.start < span.end && span.start < origin.end)
            .copied()
            .collect();
        if disposition == SourceDisposition::Eligible && !represented(span, &matches, units) {
            return Err(invalid_mapping());
        }
        let unit_indices = matches
            .into_iter()
            .map(|(index, _)| index)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        result.push(AccountingDisposition {
            accounting_index,
            disposition,
            unit_indices,
        });
    }
    if cursor != markdown.len() {
        return Err(invalid_mapping());
    }
    Ok(result)
}

/// Whether the origins of primary units cover an eligible ledger span from its start to its end
/// without a gap.
fn represented(span: SourceSpan, matches: &[(usize, SourceSpan)], units: &[SourceUnit]) -> bool {
    let mut ranges: Vec<_> = matches
        .iter()
        .filter(|(index, _)| units.get(*index).is_some_and(|unit| unit.primary))
        .map(|(_, origin)| *origin)
        .collect();
    ranges.sort_by_key(|origin| origin.start);
    let mut represented = span.start;
    for range in ranges {
        if range.start > represented {
            return false;
        }
        represented = represented.max(range.end.min(span.end));
    }
    represented == span.end
}
