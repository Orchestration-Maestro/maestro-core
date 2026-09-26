//! Batch assembly: map, split, replay and identify every authorized occurrence.
use super::batch::{ChunkBatch, ChunkDocument, RetrievalChunk};
use super::identity::{PreparedGroups, chunk_id, insert_prepared_group, prepared_identity};
use super::validation::{validate_chunks, validate_coverage};
use crate::chunk_mapping::map_document;
use crate::chunk_split::{MAX_TOKENS, TARGET_TOKENS, build_drafts};
use crate::dedup::{DedupInput, DedupScope, Deduplication, WarningPolicy, group_exact};
use crate::error::Error;
use crate::prepared_inputs::PREPARATION_PROFILE;
use crate::source_units::CHUNKER_VERSION;
use crate::tokenizer::TokenCounter;
use std::collections::BTreeMap;

/// Prepare structural chunks, counting through `counter`: it is verified before and after the
/// batch, and its contract ID enters every chunk and prepared-input identity.
/// Fresh authorization and canonical replay are required for every call.
///
/// # Errors
/// Refuses unauthorized/repeated revisions, invalid sources, disallowed warnings,
/// unsafe splits, impossible context budgets, incomplete coverage or a counter that
/// fails to verify or to count.
pub fn chunk_documents<'a>(
    scope: &'a DedupScope,
    inputs: &[DedupInput<'a>],
    warning_policy: WarningPolicy,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<ChunkBatch<'a>, Error> {
    let deduplication = group_exact(scope, inputs, warning_policy)?;
    counter.verify()?;
    let batch = build_batch(deduplication, counter.contract_id(), &mut |input| {
        Ok(counter.token_ids(input)?.len())
    })?;
    counter.verify()?;
    Ok(batch)
}

/// Map, chunk, validate and identify every authorized occurrence, counting each distinct prepared
/// input once per batch.
fn build_batch<'a>(
    deduplication: Deduplication<'a>,
    tokenizer_contract_id: &str,
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
        let doc = occurrence.document;
        let mapped = map_document(doc, occurrence.markdown)?;
        let drafts = build_drafts(doc, occurrence.markdown, &mapped, &mut cached_count)?;
        let coverage = validate_coverage(&mapped, &drafts)?;
        validate_chunks(
            doc,
            occurrence.markdown,
            &mapped,
            &drafts,
            &mut cached_count,
        )?;
        for (ordinal, content) in drafts.into_iter().enumerate() {
            let chunk_id = chunk_id(
                &deduplication,
                doc,
                &mapped,
                &content,
                tokenizer_contract_id,
            )?;
            let prepared = prepared_identity(
                &deduplication,
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
            no_searchable_content: coverage.is_empty(),
            mapped,
            coverage,
        });
    }
    Ok(ChunkBatch {
        version: CHUNKER_VERSION.into(),
        preparation_profile: PREPARATION_PROFILE.into(),
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
