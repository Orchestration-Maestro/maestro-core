//! Batch assembly: map, split, replay and identify every authorized occurrence.
use super::batch::{ChunkBatch, ChunkDocument, RetrievalChunk};
use super::identity::{PreparedGroups, chunk_id, insert_prepared_group, prepared_identity};
use super::validation::{validate_chunks, validate_coverage};
use crate::chunk_mapping::map_document;
use crate::chunk_profile::ChunkProfile;
use crate::chunk_split::{Layout, MAX_TOKENS, TARGET_TOKENS, build_drafts};
use crate::dedup::{DedupInput, DedupScope, Deduplication, WarningPolicy, group_exact};
use crate::error::Error;
use crate::tokenizer::TokenCounter;
use std::collections::BTreeMap;

/// Prepare structural chunks under `profile`, counting through `counter`: it is verified before
/// and after the batch, and its contract ID, like the profile's names, enters every chunk and
/// prepared-input identity. Fresh authorization and canonical replay are required for every call.
///
/// # Errors
/// Refuses unauthorized/repeated revisions, invalid sources, disallowed warnings,
/// unsafe splits, impossible context budgets, incomplete coverage or a counter that
/// fails to verify or to count.
pub fn chunk_documents<'a>(
    scope: &'a DedupScope,
    inputs: &[DedupInput<'a>],
    warning_policy: WarningPolicy,
    profile: ChunkProfile,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<ChunkBatch<'a>, Error> {
    let deduplication = group_exact(scope, inputs, warning_policy)?;
    counter.verify()?;
    let batch = build_batch(
        deduplication,
        counter.contract_id(),
        profile,
        &mut |input| Ok(counter.token_ids(input)?.len()),
    )?;
    counter.verify()?;
    Ok(batch)
}

/// Map, chunk, validate and identify every authorized occurrence under `profile`, counting each
/// distinct prepared input once per batch.
fn build_batch<'a>(
    deduplication: Deduplication<'a>,
    tokenizer_contract_id: &str,
    profile: ChunkProfile,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<ChunkBatch<'a>, Error> {
    // ponytail: full-string, batch-local cache; cap/evict if measured authorized
    // batches outgrow RAM.
    let mut cache = BTreeMap::new();
    let mut cached_count = |input: &str| -> Result<usize, Error> {
        if let Some(&tokens) = cache.get(input) {
            return Ok(tokens);
        }
        let tokens = count(input)?;
        cache.insert(input.to_owned(), tokens);
        Ok(tokens)
    };
    let mut documents = Vec::new();
    let mut chunks = Vec::new();
    let mut groups = PreparedGroups::new();
    for (occurrence_index, occurrence) in deduplication.occurrences.iter().enumerate() {
        let mapped = map_document(occurrence.document, occurrence.markdown)?;
        let layout = Layout::new(occurrence.document, occurrence.markdown, &mapped, profile)?;
        let drafts = build_drafts(&layout, &mut cached_count)?;
        let coverage = validate_coverage(&layout, &drafts)?;
        validate_chunks(&layout, &drafts, &mut cached_count)?;
        for (ordinal, content) in drafts.into_iter().enumerate() {
            let chunk_id = chunk_id(&deduplication, &layout, &content, tokenizer_contract_id)?;
            let prepared = prepared_identity(
                &deduplication,
                profile,
                tokenizer_contract_id,
                &content.prepared_input,
            )?;
            insert_prepared_group(
                &mut groups,
                prepared.fingerprint.clone(),
                prepared.bytes,
                prepared.group_id,
                chunks.len(),
            )?;
            chunks.push(RetrievalChunk {
                chunk_id,
                occurrence_index,
                ordinal,
                content,
                retrieval_input_fingerprint: prepared.fingerprint,
            });
        }
        documents.push(ChunkDocument {
            occurrence_index,
            no_searchable_content: coverage.iter().all(|unit| unit.primary_ranges.is_empty()),
            mapped,
            coverage,
        });
    }
    Ok(ChunkBatch {
        version: profile.chunker_version().into(),
        preparation_profile: profile.preparation_profile().into(),
        tokenizer_contract_id: tokenizer_contract_id.into(),
        target_tokens: TARGET_TOKENS,
        hard_max_tokens: MAX_TOKENS,
        overlap_tokens: 0,
        deduplication,
        documents,
        chunks,
        prepared_groups: groups.into_values().map(|(_, group)| group).collect(),
    })
}
