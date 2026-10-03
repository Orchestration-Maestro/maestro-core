//! Content and correspondence receipts converted to the existing S1 outcomes.
use super::contract::{Content, Extraction, MappedUnit, Measured, Measurement, StructureKind};
use crate::{Ref, policy::shape::valid_id};
use maestro_kernel::{acquisition::Handle, artifact::Digest, document::Outcome};
use serde::{Deserialize, Serialize};
use std::{cmp::Reverse, collections::HashMap};

/// Content-free findings; original evidence remains in the held document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "structure",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Finding {
    /// Required source measurement is absent or explicitly unknown.
    Unknown(StructureKind),
    /// Reordered, omitted, additional or substituted source units.
    Correspondence(StructureKind),
    /// Literal output content differs, including code and negation.
    Content(StructureKind),
    /// Original required asset is absent.
    Asset(StructureKind),
    /// Source span is unavailable or outside the immutable artifact.
    Provenance(StructureKind),
    /// An ancillary warning lacks its named governing rule or visible text.
    UnnamedWarning,
    /// Explicit missing-content indicators remain unresolved.
    MissingContent,
}
/// Receipt for every extraction attempt, accepted or held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FidelityReceipt {
    /// Exact separate extraction attempt.
    pub attempt: Handle,
    /// Immutable source capture handle.
    pub capture: Handle,
    /// Immutable source artifact.
    pub source: Digest,
    /// Exact selected extraction profile.
    pub profile: Ref,
    /// Exact Markdown artifact whose literal ranges were checked.
    pub markdown: Digest,
    /// Original inventories, including unknown and known zero measurements.
    pub measurements: Vec<Measurement>,
    /// Source-span correspondence and literal content receipt for every unit.
    pub units: Vec<UnitReceipt>,
    /// All findings, never only the most severe one.
    pub findings: Vec<Finding>,
}
/// One source-span mapping and its content check, not a count-based assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitReceipt {
    /// Exact source span, source content and output range checked.
    pub mapped: MappedUnit,
    /// Literal Markdown range or retained original asset matched its content.
    pub content_preserved: bool,
}
/// Immutable checked result retaining the full document and all held receipts.
#[derive(Debug)]
pub struct EvaluatedExtraction {
    /// Original attempt with every warning, missing indicator and source inventory.
    document: Extraction,
    /// All correspondence and content findings.
    receipt: FidelityReceipt,
    /// Existing S1 disposition, not a parallel admission vocabulary.
    outcome: Outcome,
}
impl EvaluatedExtraction {
    /// Original immutable attempt, also available when held.
    #[must_use]
    pub fn document(&self) -> &Extraction {
        &self.document
    }
    /// All findings bound to the source and extraction identity.
    #[must_use]
    pub fn receipt(&self) -> &FidelityReceipt {
        &self.receipt
    }
    /// Existing S1 quality outcome for canonical preparation eligibility.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        self.outcome
    }
}
/// Compare source inventories, ordered correspondence and exact output content.
/// The caller binds the capture/profile and profile-required cohorts before use;
/// adapters must not substitute output-derived inventories for source evidence.
/// Only ancillary warnings may warn-admit; source loss always holds.
#[must_use]
pub fn evaluate(document: Extraction) -> EvaluatedExtraction {
    let mut findings = Vec::new();
    for kind in &document.required {
        let measurements: Vec<_> = document
            .measurements
            .iter()
            .filter(|value| value.kind == *kind)
            .collect();
        if measurements.len() != 1 {
            findings.push(Finding::Unknown(*kind));
        }
    }
    for measurement in &document.measurements {
        compare(&document, measurement, &mut findings);
    }
    compare_physical_order(&document, &mut findings);
    let mut units = Vec::new();
    for unit in &document.units {
        if !document.measurements.iter().any(|measurement| {
            measurement.kind == unit.source.kind && matches!(measurement.source, Measured::Known(_))
        }) {
            findings.push(Finding::Unknown(unit.source.kind));
        }
        if !unit
            .source
            .span
            .is_some_and(|span| span.fits(document.source_length))
        {
            findings.push(Finding::Provenance(unit.source.kind));
        }
        let content_preserved = match &unit.source.content {
            Content::Text(text) => {
                let preserved = unit.output.and_then(|span| span.text(&document.markdown))
                    == Some(text.as_str());
                if !preserved {
                    findings.push(Finding::Content(unit.source.kind));
                }
                preserved
            }
            Content::Asset(reference) => {
                let preserved = document.assets.contains(reference);
                if !preserved {
                    findings.push(Finding::Asset(unit.source.kind));
                }
                preserved
            }
        };
        units.push(UnitReceipt {
            mapped: unit.clone(),
            content_preserved,
        });
    }
    if !document.missing.is_empty() {
        findings.push(Finding::MissingContent);
    }
    if document.warnings.iter().any(|warning| {
        !warning.rule.as_deref().is_some_and(valid_id) || warning.message.trim().is_empty()
    }) {
        findings.push(Finding::UnnamedWarning);
    }
    let outcome = outcome(&findings, !document.warnings.is_empty());
    let receipt = FidelityReceipt {
        attempt: document.attempt,
        capture: document.capture,
        source: document.source.clone(),
        profile: document.profile.clone(),
        markdown: Digest::of(document.markdown.as_bytes()),
        measurements: document.measurements.clone(),
        units,
        findings,
    };
    EvaluatedExtraction {
        document,
        receipt,
        outcome,
    }
}
/// Physical order across all kinds follows the independent source spans.
fn compare_physical_order(document: &Extraction, findings: &mut Vec<Finding>) {
    let mut source_outputs = HashMap::with_capacity(document.units.len());
    for mapped in &document.units {
        // Preserve the first exact match, including an absent output range.
        source_outputs
            .entry(&mapped.source)
            .or_insert(mapped.output);
    }
    let mut ordered = Vec::new();
    for measurement in &document.measurements {
        let Measured::Known(source) = &measurement.source else {
            continue;
        };
        for unit in source {
            let Some(span) = unit.span else {
                continue;
            };
            if let Some(output) = source_outputs.get(unit).copied().flatten() {
                ordered.push((span, output, unit.kind));
            }
        }
    }
    // A containing source span precedes its children at the same start offset.
    ordered.sort_by_key(|(source, _, _)| (source.start, Reverse(source.end)));
    for (before, after) in ordered.iter().zip(ordered.iter().skip(1)) {
        let nested = before.1.start <= after.1.start && after.1.end <= before.1.end;
        if before.1.end > after.1.start && !nested {
            findings.push(Finding::Correspondence(after.2));
        }
    }
}
/// Typed per-kind correspondence must preserve the independent inventory.
fn compare(document: &Extraction, measurement: &Measurement, findings: &mut Vec<Finding>) {
    let Measured::Known(source) = &measurement.source else {
        if document.required.contains(&measurement.kind) {
            findings.push(Finding::Unknown(measurement.kind));
        }
        return;
    };
    let mapped: Vec<_> = document
        .units
        .iter()
        .filter(|unit| unit.source.kind == measurement.kind)
        .map(|unit| &unit.source)
        .collect();
    if source.iter().collect::<Vec<_>>() != mapped {
        findings.push(Finding::Correspondence(measurement.kind));
    }
}
/// Missing provenance requires review; known loss requires re-extraction.
fn outcome(findings: &[Finding], warnings: bool) -> Outcome {
    if findings.iter().any(|finding| {
        matches!(
            finding,
            Finding::Unknown(_) | Finding::Provenance(_) | Finding::UnnamedWarning
        )
    }) {
        return Outcome::Quarantined;
    }
    if !findings.is_empty() {
        return Outcome::NeedsReextraction;
    }
    if warnings {
        Outcome::AcceptedWithWarnings
    } else {
        Outcome::Accepted
    }
}
