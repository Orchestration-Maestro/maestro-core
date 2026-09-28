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
    report::{Report, add_chrome},
    router_tokenizer::RouterTokenizer,
};
use crate::quality::eligible;
use maestro_canonicalization::{ChunkProfile, TokenCounter as _, left_out_chrome};
use maestro_kernel::{
    chunk_set::{ChunkSetState, NewChunkSet},
    document::Revision,
    gateway::ModelCard,
    scope::{Scope, ScopeSet, collection_path},
    store::Database,
};
use std::{collections::BTreeSet, ops::ControlFlow};

/// What a preparation builds: the chunk set of a collection's eligible
/// revisions, cut under a chunking profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preparation<'a> {
    /// The collection's ID.
    pub collection: &'a str,
    /// The chunking profile its revisions are cut under.
    pub profile: ChunkProfile,
}

impl<'a> Preparation<'a> {
    /// The preparation of `collection` under the default chunking profile.
    #[must_use]
    pub const fn of(collection: &'a str) -> Self {
        Self {
            collection,
            profile: ChunkProfile::Structural,
        }
    }
}

/// Prepares the collection `collection` for a caller who reads `scopes`,
/// under the default chunking profile, counting through `tokenizer`, and
/// returns its report: its eligible
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
/// The chunk set's id derives from the collection, the chunking profile,
/// the counter's contract ID and the eligible revisions, so the same input
/// gives the same chunk set and the same chunk IDs, and a new profile,
/// counter or set of revisions a new chunk set, beside the others. It completes, with its
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
    let preparation = Preparation::of(collection);
    prepare_observed(database, scopes, preparation, tokenizer, &mut unobserved)
}

/// [`prepare`] of `preparation`'s collection under its chunking profile,
/// showing `observer` the report as it stands after each batch of revisions
/// it chunks, which a job journals as its progress. When `observer` breaks,
/// as a job does once its lease is lost, the preparation stops before the
/// next batch and its chunk set stays building.
///
/// # Errors
///
/// As [`prepare`], and [`Error::Stopped`] when `observer` breaks.
pub fn prepare_observed(
    database: &Database,
    scopes: &ScopeSet,
    preparation: Preparation<'_>,
    tokenizer: &RouterTokenizer,
    observer: &mut impl FnMut(&Report) -> ControlFlow<()>,
) -> Result<Report, Error> {
    let Preparation {
        collection,
        profile,
    } = preparation;
    let (revisions, ids) = eligible_in(database, scopes, collection)?;
    let counter = tokenizer.contract_id();
    let id = id_of(collection, profile, counter, &ids);
    let set = database
        .begin_chunk_set(&NewChunkSet {
            id: &id,
            collection_id: collection,
            chunk_profile: profile.chunker_version(),
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
        profile,
    };
    let mut manifest = Manifest::new(collection, &id, profile, counter, ids);
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

/// The id of the chunk set `preparation` counted by `tokenizer` builds now,
/// for a caller who reads `scopes`: what a job that runs it freezes as its
/// input, since a revision accepted since, or another profile or counter,
/// names another chunk set. It records nothing.
///
/// # Errors
///
/// [`Error::NotVisible`] when `scopes` does not cover the collection's
/// scope, and [`Error::Records`] when the kernel cannot be read.
pub fn chunk_set_id(
    database: &Database,
    scopes: &ScopeSet,
    preparation: Preparation<'_>,
    tokenizer: &RouterTokenizer,
) -> Result<String, Error> {
    chunk_set_id_for_counter(database, scopes, preparation, tokenizer.contract_id())
}

/// The chunk set `preparation` counted by the embedder `card` would build
/// now, without qualifying the router. Its ID uses the router-counter
/// contract derived from the card's digest.
///
/// # Errors
///
/// [`Error::NotVisible`] when `scopes` does not cover the collection's
/// scope, and [`Error::Records`] when the kernel cannot be read.
pub fn chunk_set_id_for_card(
    database: &Database,
    scopes: &ScopeSet,
    preparation: Preparation<'_>,
    card: &ModelCard,
) -> Result<String, Error> {
    let counter = format!("router/1:sha256:{}", card.digest().as_str());
    chunk_set_id_for_counter(database, scopes, preparation, &counter)
}

/// The ID of the chunk set identified by `preparation`, `counter` and the
/// eligible revisions of its collection.
fn chunk_set_id_for_counter(
    database: &Database,
    scopes: &ScopeSet,
    preparation: Preparation<'_>,
    counter: &str,
) -> Result<String, Error> {
    let (_, ids) = eligible_in(database, scopes, preparation.collection)?;
    Ok(id_of(
        preparation.collection,
        preparation.profile,
        counter,
        &ids,
    ))
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
    let scope = collection_path(collection);
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
    let (resumed, pending): (Vec<&Revision>, Vec<&Revision>) = chunked
        .iter()
        .partition(|revision| done.contains(revision.id.as_str()));
    count_chrome(kernel, &resumed, manifest)?;
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
                Chunked::Recorded {
                    chunks,
                    tokens,
                    chrome,
                } => {
                    prepared += 1;
                    manifest.chunks += chunks;
                    manifest.tokens += tokens;
                    add_chrome(&mut manifest.chrome, chrome);
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

/// Counts into `manifest` the chrome the profile left out of `resumed`,
/// revisions an interrupted preparation already chunked, as their chunking
/// counted it.
fn count_chrome(
    kernel: &Kernel<'_>,
    resumed: &[&Revision],
    manifest: &mut Manifest,
) -> Result<(), Error> {
    for revision in resumed {
        // A revision it chunked loaded then, and its artifacts never change.
        let Ok(loaded) = kernel.load(revision)? else {
            continue;
        };
        let chrome = left_out_chrome(&loaded.canonical, &loaded.markdown, kernel.profile)
            .map_err(Error::Chrome)?;
        add_chrome(&mut manifest.chrome, chrome);
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
