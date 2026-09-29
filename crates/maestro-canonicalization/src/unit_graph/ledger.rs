//! Exactly-once primary ownership and explicit source exclusions.
use super::types::{
    CoverageEntry, DeliveryUnit, Exclusion, Group, MappingArtifact, MappingEntry, PartRole,
    SourcePart, SourceRange,
};
use crate::{
    chunk_mapping::mapped_slice,
    chunk_profile::{ChromeRule, ChunkProfile},
    chunk_split::Layout,
    document::CanonicalDocument,
    error::Error,
    source_units::{MappedDocument, SourceDisposition, TextRange},
};
use std::collections::{BTreeMap, BTreeSet};

/// Reconcile source accounting with exactly-once primary part ownership.
pub(super) fn coverage_ledger(units: &[DeliveryUnit]) -> Vec<CoverageEntry> {
    let mut entries: Vec<_> = units
        .iter()
        .flat_map(|unit| {
            unit.parts
                .iter()
                .filter(|part| part.role == PartRole::Primary)
                .flat_map(move |part| {
                    part.ranges.iter().map(move |range| CoverageEntry {
                        range: *range,
                        unit_id: unit.unit_id.clone(),
                        part_id: part.part_id.clone(),
                    })
                })
        })
        .collect();
    entries.sort_by_key(|entry| (entry.range.start, entry.range.end));
    entries
}

/// Create the separate canonical mapping CAS artifact from unique graph parts.
pub(super) fn mapping_artifact(
    original_markdown_digest: &str,
    units: &[DeliveryUnit],
    groups: &[Group],
    exclusions: &[Exclusion],
) -> Result<MappingArtifact, Error> {
    let mut parts = BTreeMap::<&str, &SourcePart>::new();
    for part in units
        .iter()
        .flat_map(|unit| &unit.parts)
        .filter(|part| part.role == PartRole::Primary)
        .chain(groups.iter().flat_map(|group| &group.parts))
    {
        parts.insert(part.part_id.as_str(), part);
    }
    let mut contributions = Vec::new();
    for part in parts.values() {
        if part.ranges.len() != part.mappings.len() {
            return Err(Error(
                "primary part ranges and mappings are not paired".into(),
            ));
        }
        contributions.extend(
            part.ranges
                .iter()
                .copied()
                .zip(part.mappings.iter().cloned())
                .map(|(range, mapping)| MappingEntry { range, mapping }),
        );
    }
    contributions.sort_by_key(|entry| (entry.range.start, entry.range.end));
    Ok(MappingArtifact {
        schema_version: "maestro-unit-mapping/1".into(),
        original_markdown_digest: original_markdown_digest.into(),
        contributions,
        exclusions: exclusions.to_vec(),
    })
}

/// Find primary source units excluded as complete-ideas markup chrome.
pub(super) fn chrome_markup(
    document: &CanonicalDocument,
    markdown: &str,
    mapped: &MappedDocument,
) -> Result<(BTreeSet<String>, Vec<Exclusion>), Error> {
    let layout = Layout::new(document, markdown, mapped, ChunkProfile::CompleteIdeas)?;
    let mut unit_ids = BTreeSet::new();
    let mut exclusions = Vec::new();
    for (index, unit) in mapped.units.iter().enumerate() {
        if !unit.primary
            || layout.chrome_rule(index) != Some(ChromeRule::Markup)
            || layout.kept(index).is_some()
        {
            continue;
        }
        unit_ids.insert(unit.unit_id.clone());
        for mapping in mapped_slice(
            unit,
            TextRange {
                start: 0,
                end: unit.text.len(),
            },
            markdown,
        )? {
            exclusions.extend(mapping.origins.into_iter().map(|origin| Exclusion {
                range: SourceRange {
                    start: origin.span.start,
                    end: origin.span.end,
                },
                reason: "chrome_markup".into(),
            }));
        }
    }
    exclusions.sort_by_key(|entry| (entry.range.start, entry.range.end));
    exclusions.dedup_by(|left, right| left.range == right.range);
    Ok((unit_ids, exclusions))
}

/// Keep every noneligible source-accounting entry and whole-block chrome exclusion explicit.
pub(super) fn exclusions(
    document: &CanonicalDocument,
    mapped: &MappedDocument,
    chrome: &[Exclusion],
) -> Result<Vec<Exclusion>, Error> {
    let mut exclusions: Vec<_> = mapped
        .accounting
        .iter()
        .filter(|entry| entry.disposition != SourceDisposition::Eligible)
        .map(|entry| {
            let source = document
                .source_accounting
                .get(entry.accounting_index)
                .ok_or_else(|| Error("source accounting entry is missing".into()))?;
            Ok(Exclusion {
                range: SourceRange {
                    start: source.source_span.start,
                    end: source.source_span.end,
                },
                reason: match entry.disposition {
                    SourceDisposition::Eligible => "eligible",
                    SourceDisposition::Structural => "structural",
                    SourceDisposition::Metadata => "metadata",
                    SourceDisposition::ReferenceDefinition => "reference_definition",
                }
                .into(),
            })
        })
        .collect::<Result<_, Error>>()?;
    exclusions.extend_from_slice(chrome);
    exclusions.sort_by_key(|entry| (entry.range.start, entry.range.end));
    exclusions.dedup_by(|left, right| left.range == right.range && left.reason == right.reason);
    Ok(exclusions)
}
