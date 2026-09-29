//! Build a delivery graph from checked canonical mappings and ranked views.
use super::{
    prepared::{
        PreparedInputs, prepared_text, size_limit, unit_context_parts, unit_heading_parts,
        validate_unit_sizes,
    },
    profile::{RankedUnit, UnitProfile},
    types::{
        DeliveryGraph, DeliveryUnit, GraphDescriptor, PartRole, RetrievalMembership, RetrievalView,
        SourceRange, UnitBatch, UnitGraphInput,
    },
};
use crate::{
    chunk_mapping::{map_document, mapped_slice},
    chunk_profile::ChunkProfile,
    chunks::{RetrievalChunk, chunk_documents},
    dedup::{DedupInput, DedupOccurrence, DedupScope, WarningPolicy, group_exact},
    error::Error,
    hashing::digest,
    source_units::MappedDocument,
    tokenizer::TokenCounter,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Build the opt-in /4 graph and the selected retrieval ranking views.
///
/// # Errors
/// Refuses invalid authorization, canonical mappings, counters, unsupported rules, and any
/// ambiguous or incomplete source mapping.
pub fn unit_documents<'a>(
    scope: &'a DedupScope,
    inputs: &[UnitGraphInput<'a>],
    warning_policy: WarningPolicy,
    profile: UnitProfile,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<UnitBatch<'a>, Error> {
    if !profile.size_limits().all_within_verified_counter_cap() {
        return Err(Error(
            "unit profile limits exceed the verified counter cap".into(),
        ));
    }
    counter.verify()?;
    let dedup_inputs: Vec<_> = inputs
        .iter()
        .map(|input| DedupInput {
            document: input.document,
            markdown: input.markdown,
        })
        .collect();
    let deduplication = group_exact(scope, &dedup_inputs, warning_policy)?;
    let mut graphs = Vec::with_capacity(inputs.len());
    let mut mappings = Vec::with_capacity(inputs.len());
    for occurrence in &deduplication.occurrences {
        let (graph, mapping) = build_occurrence(scope, inputs, occurrence, profile, counter)?;
        graphs.push(graph);
        mappings.push(mapping);
    }
    counter.verify()?;
    Ok(UnitBatch {
        profile,
        deduplication,
        graphs,
        mappings,
    })
}

/// Build and validate one retained source occurrence.
fn build_occurrence(
    scope: &DedupScope,
    inputs: &[UnitGraphInput<'_>],
    occurrence: &DedupOccurrence<'_>,
    profile: UnitProfile,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<(DeliveryGraph, super::types::MappingArtifact), Error> {
    let input = inputs
        .iter()
        .find(|input| {
            input.document.document_id == occurrence.document.document_id
                && input.document.revision_id == occurrence.document.revision_id
        })
        .ok_or_else(|| Error("unit graph input identity mismatch".into()))?;
    let mapped = map_document(occurrence.document, occurrence.markdown)?;
    let mut units = super::build_units::build_units(input, &mapped)?;
    super::table_packing::pack_small_tables(input, &mapped, &mut units, profile, counter)?;
    let group_records = super::groups::build_groups(input, &mut units, &mapped)?;
    let prepared = PreparedInputs {
        units: &units,
        mapped: &mapped,
        groups: &group_records.groups,
        context_relations: &group_records.context_relations,
    };
    validate_unit_sizes(&prepared, profile, counter)?;
    let view_context = ViewBuildContext {
        scope,
        input,
        mapped: &mapped,
        units: &units,
        groups: &group_records.groups,
        context_relations: &group_records.context_relations,
    };
    let retrieval_views = build_views(&view_context, profile, counter)?;
    let original_markdown_digest = occurrence
        .original_hash
        .strip_prefix("sha256:")
        .unwrap_or(&occurrence.original_hash)
        .to_owned();
    let profile_digest = profile.profile_digest()?;
    let profile_digest = profile_digest
        .strip_prefix("sha256:")
        .unwrap_or(&profile_digest)
        .to_owned();
    let preparation_digest = sha(&(
        "canonical-context-parts/v3",
        "required-context/2",
        "heading-path-from-ancestors/1",
        "rendering-separators-as-mappings/1",
    ))?;
    let coverage = super::ledger::coverage_ledger(&units);
    let exclusions = super::ledger::exclusions(occurrence.document, &mapped)?;
    let mapping = super::ledger::mapping_artifact(&original_markdown_digest, &units, &exclusions)?;
    let mapping_digest = super::serialization::mapping_digest(&mapping)?;
    let descriptor = GraphDescriptor {
        schema_version: "maestro-unit-graph/1".into(),
        collection_id: input.collection_id.into(),
        source_namespace: if input.source_namespace.trim().is_empty() {
            format!(
                "local:{}:{}",
                occurrence.document.document_id, occurrence.document.revision_id
            )
        } else {
            input.source_namespace.into()
        },
        document_id: occurrence.document.document_id.clone(),
        revision_id: occurrence.document.revision_id.clone(),
        original_markdown_digest,
        profile_name: profile.name().into(),
        profile_digest,
        preparation_name: profile.preparation_name().into(),
        preparation_digest,
        counter_contract: counter.contract_id().into(),
        graph_digest: String::new(),
        mapping_digest,
    };
    let mut graph = DeliveryGraph {
        descriptor,
        units,
        coverage,
        exclusions,
        retrieval_views,
        groups: group_records.groups,
        context_relations: group_records.context_relations,
    };
    graph.descriptor.graph_digest = super::serialization::graph_digest(&graph)?;
    super::validation::validate_graph(&graph, &mapping, occurrence.markdown)?;
    Ok((graph, mapping))
}

/// Inputs shared by both ranking-view implementations.
#[derive(Clone, Copy)]
struct ViewBuildContext<'a> {
    /// Trusted authorization scope.
    scope: &'a DedupScope,
    /// Current source occurrence.
    input: &'a UnitGraphInput<'a>,
    /// Exact mapped source.
    mapped: &'a MappedDocument,
    /// Delivery units built from the source mappings.
    units: &'a [DeliveryUnit],
    /// Structural groups whose typed relations determine context.
    groups: &'a [super::types::Group],
    /// Typed context dependencies owned by ancestor groups.
    context_relations: &'a [super::types::ContextRelationRecord],
}

/// Build retrieval views from /3 packing or the independently counted mapped units.
fn build_views(
    context: &ViewBuildContext<'_>,
    profile: UnitProfile,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<Vec<RetrievalView>, Error> {
    match profile.ranked_unit() {
        RankedUnit::CompleteIdeas => {
            let scope = DedupScope {
                tenant_id: context.scope.tenant_id.clone(),
                workspace_id: context.scope.workspace_id.clone(),
                authorized_revisions: context.scope.authorized_revisions.clone(),
            };
            let batch = chunk_documents(
                &scope,
                &[DedupInput {
                    document: context.input.document,
                    markdown: context.input.markdown,
                }],
                WarningPolicy::Preserve,
                ChunkProfile::CompleteIdeas,
                counter,
            )?;
            chunk_views(context, &batch.chunks)
        }
        RankedUnit::V2Unit => unit_views(context, profile, counter),
    }
}

/// Link each /3 fragment to its exact mapped source ranges.
fn chunk_views(
    context: &ViewBuildContext<'_>,
    chunks: &[RetrievalChunk],
) -> Result<Vec<RetrievalView>, Error> {
    let mut views = Vec::new();
    for chunk in chunks {
        let mut memberships = BTreeMap::new();
        for fragment in &chunk.content.fragments {
            let source = context
                .mapped
                .units
                .get(fragment.contribution.unit_index)
                .ok_or_else(|| Error("chunk references missing mapped unit".into()))?;
            let exact_mappings =
                mapped_slice(source, fragment.contribution.range, context.input.markdown)?;
            let mut ranges: Vec<_> = exact_mappings
                .iter()
                .flat_map(|mapping| mapping.origins.iter())
                .map(|origin| SourceRange {
                    start: origin.span.start,
                    end: origin.span.end,
                })
                .collect();
            ranges.sort_unstable();
            ranges.dedup();
            let unit = context
                .units
                .iter()
                .find(|candidate| part_uses_unit(candidate, &source.unit_id))
                .ok_or_else(|| Error("retrieval view references missing delivery unit".into()))?;
            let membership =
                memberships
                    .entry(unit.unit_id.clone())
                    .or_insert_with(|| RetrievalMembership {
                        unit_id: unit.unit_id.clone(),
                        primary_part_ids: Vec::new(),
                        primary_ranges: Vec::new(),
                        context_part_ids: Vec::new(),
                        context_ranges: Vec::new(),
                    });
            membership.primary_part_ids.extend(
                unit.parts
                    .iter()
                    .filter(|part| {
                        part.role == PartRole::Primary
                            && part
                                .mappings
                                .iter()
                                .any(|mapping| mapping.unit_id == source.unit_id)
                    })
                    .map(|part| part.part_id.clone()),
            );
            membership.primary_ranges.extend(ranges);
        }
        for membership in memberships.values_mut() {
            if let Some(unit) = context
                .units
                .iter()
                .find(|unit| unit.unit_id == membership.unit_id)
            {
                let contexts = unit_context_parts(
                    unit,
                    context.units,
                    context.groups,
                    context.context_relations,
                )?;
                membership
                    .context_part_ids
                    .extend(contexts.iter().map(|part| part.part_id.clone()));
                membership
                    .context_ranges
                    .extend(contexts.iter().flat_map(|part| part.ranges.iter().copied()));
            }
            retain_ordered(&mut membership.primary_part_ids);
            membership.primary_ranges.sort_unstable();
            membership.primary_ranges.dedup();
            retain_ordered(&mut membership.context_part_ids);
            membership.context_ranges.sort_unstable();
            membership.context_ranges.dedup();
        }
        let mut memberships: Vec<_> = memberships.into_values().collect();
        memberships.sort_by_key(|membership| {
            context
                .units
                .iter()
                .position(|unit| unit.unit_id == membership.unit_id)
                .unwrap_or(usize::MAX)
        });
        let view_id = format!("view-{}", sha(&chunk.chunk_id)?);
        views.push(RetrievalView {
            rank_policy: "complete_ideas".into(),
            retrieval_view_id: view_id,
            chunk_id: chunk.chunk_id.clone(),
            prepared_input_digest: chunk
                .retrieval_input_fingerprint
                .strip_prefix("sha256:")
                .unwrap_or(&chunk.retrieval_input_fingerprint)
                .to_owned(),
            token_count: chunk.content.token_count,
            memberships,
        });
    }
    Ok(views)
}

/// Build one counted V2 retrieval view per mapped delivery unit.
fn retain_ordered<T: Ord + Clone>(values: &mut Vec<T>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

/// Build one counted V2 retrieval view per mapped delivery unit.
fn unit_views(
    context: &ViewBuildContext<'_>,
    profile: UnitProfile,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<Vec<RetrievalView>, Error> {
    let mut views = Vec::new();
    for delivery in context.units {
        let contexts = unit_context_parts(
            delivery,
            context.units,
            context.groups,
            context.context_relations,
        )?;
        let headings = unit_heading_parts(delivery, context.units, context.groups)?;
        let prepared_input = prepared_text(delivery, context.mapped, &contexts, &headings)?;
        let tokens = counter.token_ids(&prepared_input)?.len();
        if tokens > size_limit(profile, delivery.kind) {
            return Err(Error(format!(
                "oversized_unit_refusal: {} has no mapped structural continuation",
                delivery.unit_id
            )));
        }
        let prepared_input_digest = digest(prepared_input.as_bytes());
        let chunk_id = format!(
            "chunk-{}",
            sha(&(
                &context.input.document.document_id,
                &context.input.document.revision_id,
                &delivery.unit_id,
                &prepared_input_digest
            ))?
        );
        views.push(RetrievalView {
            rank_policy: "v2_unit".into(),
            retrieval_view_id: format!("view-{}", sha(&chunk_id)?),
            chunk_id,
            prepared_input_digest,
            token_count: tokens,
            memberships: vec![RetrievalMembership {
                unit_id: delivery.unit_id.clone(),
                primary_part_ids: delivery
                    .parts
                    .iter()
                    .filter(|part| part.role == PartRole::Primary)
                    .map(|part| part.part_id.clone())
                    .collect(),
                primary_ranges: delivery
                    .parts
                    .iter()
                    .filter(|part| part.role == PartRole::Primary)
                    .flat_map(|part| part.ranges.iter().copied())
                    .collect(),
                context_part_ids: contexts.iter().map(|part| part.part_id.clone()).collect(),
                context_ranges: contexts
                    .iter()
                    .flat_map(|part| part.ranges.iter().copied())
                    .collect(),
            }],
        });
    }
    Ok(views)
}

/// Find a delivery unit containing a mapping from `unit_id`.
fn part_uses_unit(unit: &DeliveryUnit, unit_id: &str) -> bool {
    unit.parts
        .iter()
        .filter(|part| part.role == PartRole::Primary)
        .flat_map(|part| &part.mappings)
        .any(|mapping| mapping.unit_id == unit_id)
}

/// Hash deterministic JSON identity bytes.
fn sha(value: &impl Serialize) -> Result<String, Error> {
    let bytes = serde_json::to_vec(value).map_err(|error| Error(error.to_string()))?;
    Ok(digest(&bytes))
}
