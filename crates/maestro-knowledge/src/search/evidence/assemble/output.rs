//! Builds trace and gaps, validates the bundle, and rechecks authority.

use super::super::{
    budget::{self, CounterInfo},
    delivery_graph::PrimaryContribution,
    signals::{GapInput, SourceWarning, TraceInput, build_known_gaps, trace_for_passage},
    spans::SeedSpan,
    types::EvidenceError,
};
use super::{
    engine::{AssemblyWorker, check, integrity},
    types::BundleParts,
};
use crate::query::Language;
use maestro_canonicalization::Severity;
use maestro_kernel::{
    document::Outcome,
    evidence::{Budget, Bundle, Passage, Schema, Trace},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::sink,
};

impl AssemblyWorker<'_> {
    /// Builds signals, validates the wire bundle, and performs final authority checks.
    pub(super) fn finish(
        &self,
        counter_info: CounterInfo,
        parts: BundleParts<'_>,
    ) -> Result<Bundle, EvidenceError> {
        let trace = self.traces(
            &parts.passages,
            &parts.seeds,
            &parts.parent_supports,
            &parts.primary_contributions,
        )?;
        let warning_codes = self.warning_codes(&parts.passages)?;
        let source_warnings = self.source_warnings(&parts.passages, &warning_codes)?;
        let identifiers = self
            .input
            .understood
            .identifiers
            .iter()
            .map(|identifier| identifier.text.clone())
            .collect::<Vec<_>>();
        let known_gaps = build_known_gaps(GapInput {
            inherited: self.input.known_gaps.clone(),
            passages: &parts.passages,
            has_inventory: self.input.inventory.is_some(),
            unresolved_candidate: self.unresolved_candidate,
            identifiers: &identifiers,
            candidate_texts: &parts.candidate_texts,
            requested_version: self.input.version.as_deref(),
            latest_undetermined: parts.latest_undetermined,
            omissions: parts.omissions,
            within_passage_conflicts: &parts.emission.within_passage,
            source_warnings: &source_warnings,
        })
        .map_err(|_| integrity("known evidence gaps could not be constructed"))?;
        check(self.control)?;
        let evidence_bytes = budget::count_passages(
            &parts.passages,
            self.counter,
            &counter_info,
            self.input.budget.evidence_bytes,
        )
        .map_err(EvidenceError::from)?;
        check(self.control)?;
        budget::verify_counter(self.counter, &counter_info).map_err(EvidenceError::from)?;
        check(self.control)?;

        let bundle = Bundle {
            schema: Schema::V1,
            collection: self.generation.collection_id.clone(),
            generation: self.generation.id,
            query: self.input.query.clone(),
            lang: language_name(self.input.understood.language).to_owned(),
            routes: self.input.routes.clone(),
            passages: parts.passages,
            conflicts: parts.emission.conflicts,
            known_gaps,
            budget: Budget {
                evidence_bytes,
                limit: self.input.budget.evidence_bytes,
                counter: counter_info.counter,
                estimated: counter_info.estimated,
            },
            request_budget: Some(self.input.budget),
            inventory: self.input.inventory.clone(),
            trace,
        };
        serde_json::to_writer(sink(), &bundle).map_err(EvidenceError::Json)?;
        check(self.control)?;
        self.recheck_survivors(&bundle.passages)?;
        let visible = self
            .database
            .visible(&self.input.principal)
            .map_err(EvidenceError::Store)?;
        check(self.control)?;
        if visible != *self.input.scopes {
            return Err(EvidenceError::PermissionsChanged);
        }
        Ok(bundle)
    }

    /// Constructs trace entries using only source seeds covered by each passage.
    fn traces(
        &self,
        passages: &[Passage],
        seeds: &[SeedSpan],
        supports: &BTreeMap<u32, Vec<String>>,
        primary: &BTreeMap<u32, Vec<PrimaryContribution>>,
    ) -> Result<Vec<Trace>, EvidenceError> {
        passages
            .iter()
            .map(|passage| {
                check(self.control)?;
                let source = self
                    .sources
                    .get(&passage.revision_id)
                    .ok_or_else(|| integrity("passage source was not cached"))?;
                trace_for_passage(&TraceInput {
                    primary: primary.get(&passage.n).map_or(&[], Vec::as_slice),
                    parent_context_of: supports.get(&passage.n).map_or(&[], Vec::as_slice),
                    number: passage.n,
                    revision_id: &passage.revision_id,
                    span: passage.span,
                    seeds,
                    document: &source.canonical,
                    markdown: &source.markdown,
                })
                .map_err(|_| integrity("passage trace could not be constructed"))
            })
            .collect()
    }

    /// Collects authorized warning codes once per returned revision.
    fn warning_codes(
        &self,
        passages: &[Passage],
    ) -> Result<BTreeMap<String, Vec<String>>, EvidenceError> {
        let revisions: BTreeSet<_> = passages
            .iter()
            .map(|passage| passage.revision_id.as_str())
            .collect();
        let mut codes = BTreeMap::new();
        for revision in revisions {
            let source = self
                .sources
                .get(revision)
                .ok_or_else(|| integrity("warning source was not cached"))?;
            if source.disposition.outcome == Outcome::AcceptedWithWarnings {
                codes.insert(
                    revision.to_owned(),
                    source
                        .canonical
                        .warnings
                        .iter()
                        .filter(|finding| finding.severity == Severity::Warning)
                        .map(|finding| finding.code.clone())
                        .collect(),
                );
            }
        }
        Ok(codes)
    }

    /// Attaches source warning details to each passage from a warned revision.
    fn source_warnings<'b>(
        &'b self,
        passages: &[Passage],
        codes: &'b BTreeMap<String, Vec<String>>,
    ) -> Result<Vec<SourceWarning<'b>>, EvidenceError> {
        let mut warnings = Vec::new();
        for passage in passages {
            let source = self
                .sources
                .get(&passage.revision_id)
                .ok_or_else(|| integrity("warning source was not cached"))?;
            if source.disposition.outcome == Outcome::AcceptedWithWarnings {
                warnings.push(SourceWarning {
                    passage_number: passage.n,
                    disposition_reasons: &source.disposition.reasons,
                    rule_ids: &source.disposition.rule_ids,
                    warning_codes: codes.get(&passage.revision_id).map_or(&[], Vec::as_slice),
                });
            }
        }
        Ok(warnings)
    }

    /// Re-reads scoped eligibility without rereading immutable artifacts.
    fn recheck_survivors(&self, passages: &[Passage]) -> Result<(), EvidenceError> {
        let revisions: BTreeSet<_> = passages
            .iter()
            .map(|passage| passage.revision_id.as_str())
            .collect();
        for revision_id in revisions {
            check(self.control)?;
            let live_revision = self
                .database
                .revision(&self.input.scopes, revision_id)
                .map_err(EvidenceError::Records)?
                .ok_or(EvidenceError::NotVisible)?;
            check(self.control)?;
            let live_disposition = self
                .database
                .disposition(&self.input.scopes, revision_id)
                .map_err(EvidenceError::Records)?
                .ok_or(EvidenceError::NotVisible)?;
            check(self.control)?;
            let cached = self
                .sources
                .get(revision_id)
                .ok_or_else(|| integrity("returned revision was not cached"))?;
            if live_revision != cached.revision || live_disposition != cached.disposition {
                return Err(integrity("evidence eligibility changed during assembly"));
            }
        }
        Ok(())
    }
}

/// Maps the shared query language to its compact evidence tag.
fn language_name(language: Language) -> &'static str {
    match language {
        Language::French => "fr",
        Language::English => "en",
        Language::Unknown => "und",
    }
}
