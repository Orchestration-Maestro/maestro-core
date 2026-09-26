//! Preparing a collection: its eligible revisions deduplicated, then chunked
//! into the chunk set their identity names, which completes with its
//! manifest.

use super::{
    chunking::{BATCH, Chunked},
    counter::Counting,
    error::TokenizerError,
    exact::{Kernel, exact_groups, occurrences},
    failure::Error,
    manifest::{Manifest, id_of},
    near::{Shingles, Vocabulary, groups},
    report::Report,
    router_tokenizer::RouterTokenizer,
};
use crate::quality::eligible;
use maestro_canonicalization::{CHUNKER_VERSION, TokenCounter as _};
use maestro_kernel::{
    chunk_set::{ChunkSetState, NewChunkSet},
    document::Revision,
    scope::{Scope, ScopeSet},
    store::Database,
};
use std::{collections::BTreeSet, ops::ControlFlow};

/// Prepares the collection `collection` for a caller who reads `scopes`,
/// counting through `tokenizer`, and returns its report: its eligible
/// revisions deduplicated and chunked into a chunk set (docs/architecture/01
/// §6 and §7). A document is prepared by its latest revision in record order
/// alone, when the quality gate lets it through
/// ([`quality::eligible`](crate::quality::eligible)): not failed, and
/// accepted, with or without warnings. A document whose latest revision is
/// held back, failed or not decided yet is left out, with no older revision
/// in its place, and the report and the manifest name it with why.
///
/// Exact duplicates are prepared once, as the one with the smallest revision
/// ID, and every place each occurs is recorded as an occurrence of it; near
/// duplicates are grouped with their confirmed Jaccard, and each is
/// prepared. A revision the chunker refuses, such as one with a unit that
/// cannot fit 700 tokens with its context, gets no chunk, and its refusal is
/// reported and recorded with the chunk set; the rest are prepared.
///
/// The chunk set's id derives from the collection, the chunker's profile,
/// the counter's contract ID and the eligible revisions, so the same input
/// gives the same chunk set and the same chunk IDs, and a new profile,
/// counter or set of revisions a new chunk set. It completes, with its
/// manifest, only once every eligible revision is settled: interrupted work
/// leaves it building, and a rerun resumes it where it stopped, while a
/// rerun of a complete set changes nothing.
///
/// # Errors
///
/// Before any work, [`Error::NotVisible`] when `scopes` does not cover the
/// collection's scope, and [`Error::Failed`] when its chunk set failed
/// before. Part way, [`Error::Counter`] when the counter refused, which
/// fails the chunk set only when the counter changed under its card, and
/// [`Error::Records`], [`Error::ChunkSet`], [`Error::Artifacts`] or
/// [`Error::Manifest`] when the kernel fails: what was recorded stays.
pub fn prepare(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    tokenizer: &RouterTokenizer,
) -> Result<Report, Error> {
    let mut unobserved = |_: &Report| ControlFlow::Continue(());
    prepare_observed(database, scopes, collection, tokenizer, &mut unobserved)
}

/// [`prepare`], showing `observer` the report as it stands after each batch
/// of revisions it chunks, which a job journals as its progress. When
/// `observer` breaks, as a job does once its lease is lost, the preparation
/// stops before the next batch and its chunk set stays building.
///
/// # Errors
///
/// As [`prepare`], and [`Error::Stopped`] when `observer` breaks.
pub fn prepare_observed(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    tokenizer: &RouterTokenizer,
    observer: &mut impl FnMut(&Report) -> ControlFlow<()>,
) -> Result<Report, Error> {
    let (revisions, ids) = eligible_in(database, scopes, collection)?;
    let counter = tokenizer.contract_id();
    let id = id_of(collection, counter, &ids);
    let set = database
        .begin_chunk_set(&NewChunkSet {
            id: &id,
            collection_id: collection,
            chunk_profile: CHUNKER_VERSION,
            counter_contract_id: counter,
        })
        .map_err(Error::ChunkSet)?;
    match (set.state, &set.manifest_digest) {
        (ChunkSetState::Failed, _) => return Err(Error::Failed(id)),
        (ChunkSetState::Complete, Some(digest)) => {
            return Ok(Manifest::read(database, digest)?.final_report());
        }
        _ => {}
    }
    let kernel = Kernel {
        database,
        scopes,
        collection,
    };
    let mut manifest = Manifest::new(collection, &id, counter, ids);
    manifest.left_out = kernel.left_out(&revisions)?;
    let chunked = deduplicate(&kernel, &revisions, &mut manifest)?;
    chunk_all(&kernel, tokenizer, &chunked, &mut manifest, observer)?;
    manifest
        .refusals
        .sort_by(|first, second| first.revision.cmp(&second.revision));
    let digest = manifest.store(database)?;
    database
        .complete_chunk_set(&id, &digest)
        .map_err(Error::ChunkSet)?;
    Ok(manifest.final_report())
}

/// The id of the chunk set a preparation of the collection `collection`
/// counted by `tokenizer` builds now, for a caller who reads `scopes`: what a
/// job that runs it freezes as its input, since a revision accepted since,
/// or another profile or counter, names another chunk set. It records
/// nothing.
///
/// # Errors
///
/// [`Error::NotVisible`] when `scopes` does not cover the collection's
/// scope, and [`Error::Records`] when the kernel cannot be read.
pub fn chunk_set_id(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    tokenizer: &RouterTokenizer,
) -> Result<String, Error> {
    let (_, ids) = eligible_in(database, scopes, collection)?;
    Ok(id_of(collection, tokenizer.contract_id(), &ids))
}

/// The eligible revisions of the collection `collection`, as the quality gate
/// lets them through, in ID order, and their IDs, for a caller who reads
/// `scopes`.
///
/// # Errors
///
/// [`Error::NotVisible`] when `scopes` does not cover the collection's
/// scope, and [`Error::Records`] when the kernel cannot be read.
fn eligible_in(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
) -> Result<(Vec<Revision>, Vec<String>), Error> {
    let scope = format!("workspace/default/collection/{collection}");
    if !scope
        .parse::<Scope>()
        .is_ok_and(|parsed| scopes.covers(&parsed))
    {
        return Err(Error::NotVisible(scope));
    }
    let mut revisions = eligible(database, scopes, collection).map_err(Error::Records)?;
    revisions.sort_by(|first, second| first.id.cmp(&second.id));
    let ids = revisions
        .iter()
        .map(|revision| revision.id.clone())
        .collect();
    Ok((revisions, ids))
}

/// Fingerprints each of `revisions`, groups its exact duplicates and records
/// their occurrences, then groups the near duplicates among those it
/// prepares, into `manifest`: the revisions to chunk, one per group of exact
/// duplicates, in ID order.
fn deduplicate(
    kernel: &Kernel<'_>,
    revisions: &[Revision],
    manifest: &mut Manifest,
) -> Result<Vec<Revision>, Error> {
    let mut vocabulary = Vocabulary::default();
    let mut fingerprints = Vec::new();
    for revision in revisions {
        match kernel.fingerprint(revision, &mut vocabulary)? {
            Ok(fingerprint) => fingerprints.push(fingerprint),
            Err(refusal) => manifest.refusals.push(refusal),
        }
    }
    let exact = exact_groups(fingerprints);
    let mut places = Vec::new();
    for group in &exact {
        places.extend(occurrences(kernel.collection, group));
        if let Some((chunked, duplicates)) = group.split_first() {
            for duplicate in duplicates {
                let (duplicate, chunked) = (&duplicate.revision.id, &chunked.revision.id);
                manifest
                    .duplicates
                    .insert(duplicate.clone(), chunked.clone());
            }
        }
    }
    kernel
        .database
        .record_occurrences(&places)
        .map_err(Error::Records)?;
    let shingled: Vec<(&str, &Shingles)> = exact
        .iter()
        .filter_map(|group| group.first())
        .map(|chunked| (chunked.revision.id.as_str(), &chunked.shingles))
        .collect();
    let near = groups(&shingled);
    kernel
        .database
        .record_near_duplicates(&near.concat())
        .map_err(Error::Records)?;
    manifest.near_duplicate_groups = near
        .iter()
        .filter_map(|group| Some(group.first()?.group_id.clone()))
        .collect();
    Ok(exact
        .into_iter()
        .filter_map(|group| Some(group.into_iter().next()?.revision))
        .collect())
}

/// Chunks each of `chunked` the chunk set of `manifest` holds no chunk of
/// yet, [`BATCH`] at a time, counting through `tokenizer`, and settles each
/// in `manifest`; `observer` sees the report after each batch.
fn chunk_all(
    kernel: &Kernel<'_>,
    tokenizer: &RouterTokenizer,
    chunked: &[Revision],
    manifest: &mut Manifest,
    observer: &mut impl FnMut(&Report) -> ControlFlow<()>,
) -> Result<(), Error> {
    let chunk_set = manifest.chunk_set.clone();
    let recorded = kernel
        .database
        .chunks(kernel.scopes, &chunk_set)
        .map_err(Error::ChunkSet)?;
    let done: BTreeSet<&str> = recorded
        .iter()
        .map(|chunk| chunk.revision_id.as_str())
        .collect();
    manifest.chunks = u64::try_from(recorded.len()).unwrap_or(u64::MAX);
    manifest.tokens = recorded.iter().map(|chunk| chunk.token_count).sum();
    let mut prepared = u64::try_from(done.len()).unwrap_or(u64::MAX);
    let pending: Vec<&Revision> = chunked
        .iter()
        .filter(|revision| !done.contains(revision.id.as_str()))
        .collect();
    let counting = Counting::new(tokenizer);
    for batch in pending.chunks(BATCH) {
        let mut loaded = Vec::new();
        for revision in batch {
            match kernel.load(revision)? {
                Ok(read) => loaded.push(read),
                Err(refusal) => manifest.refusals.push(refusal),
            }
        }
        let outcomes = kernel
            .chunk(&chunk_set, &counting, &loaded)
            .or_else(|error| fail_on_disagreement(kernel.database, &chunk_set, error))?;
        for outcome in outcomes {
            match outcome {
                Chunked::Recorded { chunks, tokens } => {
                    prepared += 1;
                    manifest.chunks += chunks;
                    manifest.tokens += tokens;
                }
                Chunked::Refused(refusal) => manifest.refusals.push(refusal),
            }
        }
        if observer(&manifest.report(prepared)).is_break() {
            return Err(Error::Stopped);
        }
    }
    Ok(())
}

/// `error`, once the chunk set `chunk_set` has failed when it is a counter
/// whose canaries changed under its card: its counts are not trusted.
fn fail_on_disagreement<T>(database: &Database, chunk_set: &str, error: Error) -> Result<T, Error> {
    if matches!(error, Error::Counter(TokenizerError::Disagreement { .. })) {
        database
            .fail_chunk_set(chunk_set)
            .map_err(Error::ChunkSet)?;
    }
    Err(error)
}
