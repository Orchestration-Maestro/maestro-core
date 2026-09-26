//! Chunking prepared revisions with maestro-canonicalization's chunker,
//! counted through the router tokenizer, into a building chunk set: each
//! chunk's exact prepared input stored as an artifact, and each revision's
//! chunks recorded at once, pinning those artifacts.
//!
//! The chunker refuses a whole call when it refuses one of its documents, so
//! a call it refuses is made again for each revision alone: the revision it
//! refuses then gets its refusal and no chunk, and the rest are chunked. A
//! refusal of the counter is no refusal of a document: it stops the batch.

use super::{counter::Counting, exact::Kernel, exact::Loaded, failure::Error, report::Refusal};
use maestro_canonicalization::{
    ChunkBatch, ChunkContent, DedupInput, InputRole, WarningPolicy, chunk_documents,
};
use maestro_kernel::{artifact::Digest, chunk_set::Chunk, evidence::Span};
use std::slice;

/// How many revisions one call of the chunker prepares: the counter's
/// canaries are counted again before and after each call, so a batch spreads
/// that cost over its revisions.
pub(super) const BATCH: usize = 16;

/// The media type of a prepared input's artifact.
const PREPARED_INPUT: &str = "text/plain; charset=utf-8";

/// What chunking one revision did.
#[derive(Debug)]
pub(super) enum Chunked {
    /// Its chunks are recorded: how many, and their tokens.
    Recorded {
        /// Its chunks.
        chunks: u64,
        /// The tokens of their prepared inputs.
        tokens: u64,
    },
    /// The chunker refused it, and it got no chunk.
    Refused(Refusal),
}

impl Kernel<'_> {
    /// Chunks `batch` into the building chunk set `chunk_set`, counting
    /// through `counting`, and records each revision's chunks: what became of
    /// each revision, in the order of `batch`.
    ///
    /// # Errors
    ///
    /// [`Error::Counter`] when the counter refused, [`Error::Artifacts`] and
    /// [`Error::ChunkSet`] when the kernel cannot store or record the chunks.
    pub(super) fn chunk(
        &self,
        chunk_set: &str,
        counting: &Counting<'_>,
        batch: &[Loaded],
    ) -> Result<Vec<Chunked>, Error> {
        let loaded: Vec<&Loaded> = batch.iter().collect();
        let scope = self.dedup_scope(&loaded);
        let inputs: Vec<DedupInput<'_>> = batch
            .iter()
            .map(|loaded| DedupInput {
                document: &loaded.canonical,
                markdown: &loaded.markdown,
            })
            .collect();
        // A refusal of a batch before this one stopped it: none is left.
        counting.take_refusal();
        let refused = match chunk_documents(&scope, &inputs, WarningPolicy::Preserve, counting) {
            Ok(chunked) => return self.record(chunk_set, batch, &chunked),
            Err(refused) => refused,
        };
        if let Some(refusal) = counting.take_refusal() {
            return Err(Error::Counter(refusal));
        }
        if let [alone] = batch {
            return Ok(vec![Chunked::Refused(alone.refusal(refused.0))]);
        }
        let mut outcomes = Vec::new();
        for alone in batch {
            outcomes.extend(self.chunk(chunk_set, counting, slice::from_ref(alone))?);
        }
        Ok(outcomes)
    }

    /// Stores the prepared input of each chunk of `chunked`, cut from
    /// `batch`, and records each revision's chunks in `chunk_set`: what
    /// became of each revision of `batch`, in its order. A revision the
    /// chunker gave no occurrence is refused.
    fn record(
        &self,
        chunk_set: &str,
        batch: &[Loaded],
        chunked: &ChunkBatch<'_>,
    ) -> Result<Vec<Chunked>, Error> {
        let mut outcomes = Vec::new();
        for loaded in batch {
            let occurrence = chunked
                .deduplication
                .occurrences
                .iter()
                .position(|occurrence| occurrence.document.revision_id == loaded.revision.id);
            let Some(index) = occurrence else {
                let reason = "the chunker gave it no occurrence".to_owned();
                outcomes.push(Chunked::Refused(loaded.refusal(reason)));
                continue;
            };
            let contents: Vec<(&str, &ChunkContent)> = chunked
                .chunks
                .iter()
                .filter(|chunk| chunk.occurrence_index == index)
                .map(|chunk| (chunk.chunk_id.as_str(), &chunk.content))
                .collect();
            outcomes.push(self.record_revision(chunk_set, loaded, &contents)?);
        }
        Ok(outcomes)
    }

    /// Stores the prepared input of each of `contents`, the chunks of
    /// `loaded` by their ids, then records them in `chunk_set` at once; a
    /// chunk that covers no source byte refuses the revision.
    fn record_revision(
        &self,
        chunk_set: &str,
        loaded: &Loaded,
        contents: &[(&str, &ChunkContent)],
    ) -> Result<Chunked, Error> {
        let mut spans = Vec::new();
        for (id, content) in contents {
            let Some(span) = span(content) else {
                let reason = format!("its chunk {id} covers no byte of its original");
                return Ok(Chunked::Refused(loaded.refusal(reason)));
            };
            spans.push(span);
        }
        let mut chunks = Vec::new();
        let mut tokens = 0;
        for ((id, content), span) in contents.iter().zip(spans) {
            let digest = self.store(&content.prepared_input)?;
            let token_count = u64::try_from(content.token_count).unwrap_or(u64::MAX);
            tokens += token_count;
            chunks.push(Chunk {
                id: (*id).to_owned(),
                revision_id: loaded.revision.id.clone(),
                section_id: content.section_id.clone(),
                digest,
                token_count,
                span,
            });
        }
        self.database
            .record_chunks(chunk_set, &loaded.revision.id, &chunks)
            .map_err(Error::ChunkSet)?;
        Ok(Chunked::Recorded {
            chunks: u64::try_from(chunks.len()).unwrap_or(u64::MAX),
            tokens,
        })
    }

    /// Stores `prepared_input` as an artifact, and returns its digest.
    fn store(&self, prepared_input: &str) -> Result<Digest, Error> {
        self.database
            .put(prepared_input.as_bytes(), PREPARED_INPUT)
            .map_err(Error::Artifacts)
    }
}

/// The bytes of its revision's original `content` covers: from the first to
/// the last byte its source content comes from, none when it comes from none.
fn span(content: &ChunkContent) -> Option<Span> {
    let origins = content
        .input_parts
        .iter()
        .filter(|part| part.role == InputRole::SourceContent)
        .flat_map(|part| &part.mappings)
        .flat_map(|run| &run.origins);
    let start = origins.clone().map(|origin| origin.span.start).min()?;
    let end = origins.map(|origin| origin.span.end).max()?;
    Some(Span { start, end })
}
