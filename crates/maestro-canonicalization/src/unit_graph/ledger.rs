//! Exactly-once primary ownership and explicit source exclusions.
use super::types::{
    CoverageEntry, DeliveryUnit, Exclusion, MappingArtifact, MappingEntry, PartRole, SourceRange,
};
use crate::{
    document::CanonicalDocument,
    error::Error,
    source_units::{MappedDocument, SourceDisposition},
};

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

/// Create the separate canonical mapping CAS artifact from primary part mappings.
pub(super) fn mapping_artifact(
    original_markdown_digest: &str,
    units: &[DeliveryUnit],
    exclusions: &[Exclusion],
) -> Result<MappingArtifact, Error> {
    let mut contributions = Vec::new();
    for part in units
        .iter()
        .flat_map(|unit| &unit.parts)
        .filter(|part| part.role == PartRole::Primary)
    {
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

/// Keep every noneligible source-accounting entry with an explicit exclusion reason.
pub(super) fn exclusions(
    document: &CanonicalDocument,
    mapped: &MappedDocument,
) -> Result<Vec<Exclusion>, Error> {
    mapped
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
        .collect()
}
