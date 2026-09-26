//! What a preparation reports: the chunk set it built, how many eligible
//! revisions it prepared, left as duplicates or refused, the groups of near
//! duplicates, the chunks and their tokens, each refusal with its reason,
//! and each document it left out with why.

use serde::{Deserialize, Serialize};

/// What a preparation reports, as JSON: the counts, then the refusals and the
/// documents left out. Every document with a revision is counted once, as
/// `eligible` or `left_out`, and every eligible revision once: `eligible` is
/// `prepared` plus `duplicates` plus `refused`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// The collection prepared.
    pub collection: String,
    /// The chunk set it built.
    pub chunk_set: String,
    /// The chunker's profile, maestro-canonicalization's own identifier.
    pub chunk_profile: String,
    /// The contract ID of the counter that counted the chunks.
    pub counter: String,
    /// The collection's eligible revisions: of each document, its latest
    /// revision in record order, when it is not failed and is accepted, with
    /// or without warnings.
    pub eligible: u64,
    /// The collection's documents whose latest revision is not eligible,
    /// which it leaves out: no older revision stands in for it.
    pub left_out: u64,
    /// The eligible revisions whose content it chunked: of each group of the
    /// same content, its smallest revision, unless it is refused, when the
    /// group adds none here.
    pub prepared: u64,
    /// The eligible revisions left as exact duplicates: each group of the
    /// same content is prepared once, as its smallest revision, and the
    /// others count here, their places kept as occurrences of it, even when
    /// that revision is refused.
    pub duplicates: u64,
    /// The groups of near duplicates among the smallest revisions of the
    /// groups of the same content, found before chunking: a member may be
    /// refused.
    pub near_duplicate_groups: u64,
    /// The chunks the chunk set holds.
    pub chunks: u64,
    /// The tokens of their prepared inputs.
    pub tokens: u64,
    /// The eligible revisions it refused to chunk.
    pub refused: u64,
    /// Why, for each, in revision order.
    pub refusals: Vec<Refusal>,
    /// Each document left out, with its latest revision and why, in document
    /// order.
    pub left_out_documents: Vec<LeftOut>,
}

/// An eligible revision the preparation refused to chunk, with the reason:
/// a unit that does not fit 700 tokens with its context names the unit, its
/// block and the block's span, never its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    /// The revision refused.
    pub revision: String,
    /// Its document.
    pub document: String,
    /// Where the document comes from.
    pub source_ref: String,
    /// Why, as the chunker or the kernel gave it.
    pub reason: String,
}

/// A document of the collection a preparation leaves out, since its latest
/// revision in record order is not eligible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeftOut {
    /// The document.
    pub document: String,
    /// Where it comes from.
    pub source_ref: String,
    /// Its latest revision in record order.
    pub revision: String,
    /// Why that revision is not eligible.
    pub reason: Ineligibility,
}

/// Why a document's latest revision is not eligible, which the JSON gives by
/// name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ineligibility {
    /// Canonicalization failed on it, so it is never eligible, whatever its
    /// disposition.
    Failed,
    /// The quality gate holds it back for another extraction.
    NeedsReextraction,
    /// The quality gate holds it back for a review.
    Quarantined,
    /// The quality gate excludes it.
    Excluded,
    /// It had no disposition when the eligible revisions were read.
    Undecided,
}
