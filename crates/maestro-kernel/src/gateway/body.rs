//! Bounded answers: each call reads at most what its endpoint can
//! legitimately answer, with headroom, so a misbehaving router or model
//! server cannot exhaust memory. A declared length over the limit is refused
//! before any byte is read, and a body without one stops at the limit.

use super::port::Error;
use reqwest::Response;

/// Maximum buffered HTTP chat body, including JSON escaping and metadata.
pub(super) const MAX_CHAT_BODY_BYTES: usize = 262_144;

/// Maximum `/props` body. llama.cpp's server reports its chat template and,
/// for some models, a second tool-use template: 2 templates × 128 KiB (the
/// largest known template is about 20 KiB) × 2 for JSON escaping is 512 KiB,
/// and the generation settings and the rest are under 64 KiB; the limit
/// doubles that to 1 MiB.
pub(super) const MAX_PROPS_BODY_BYTES: usize = 1_048_576;

/// Maximum `/v1/models` body: 1024 entries × 1 KiB per listed entry (its name,
/// owner and status run to a few hundred bytes).
pub(super) const MAX_CATALOG_BODY_BYTES: usize = 1_048_576;

/// Maximum refusal body. The router's and llama.cpp's refusals are one
/// envelope with a code, a type and a message of at most a few hundred
/// bytes; 64 KiB leaves room for a message that quotes a long value.
pub(super) const MAX_ERROR_BODY_BYTES: usize = 65_536;

/// The largest formatted token ID, `4294967295`, with its separator.
const BYTES_PER_TOKEN: usize = 11;

/// The special tokens a tokenization adds at the ends of the text.
const ADDED_TOKENS: usize = 16;

/// The largest formatted vector component: llama.cpp writes each `f32` as the
/// shortest round trip of its `f64` widening, at most 23 characters such as
/// `-1.1754943508222875e-38`, then a separator; 32 leaves room for spacing.
const BYTES_PER_COMPONENT: usize = 32;

/// One item of a list answer without its vector: `{"index": …, "object":
/// "embedding", "embedding": []}` or `{"index": …, "relevance_score": …}`
/// run under 100 bytes.
const BYTES_PER_ITEM: usize = 256;

/// What surrounds a list answer's items: the model's name, the object type
/// and the usage counts, a few hundred bytes.
const ENVELOPE_BYTES: usize = 65_536;

/// Maximum `/tokenize` body for a text of `text_bytes` bytes:
/// (`text_bytes` + [`ADDED_TOKENS`]) × [`BYTES_PER_TOKEN`] + [`ENVELOPE_BYTES`].
///
/// This is a budget of answer bytes, not a bound of one token per byte: a
/// normalization can give more tokens than bytes. BGE-M3's embedded map
/// expands U+FDFA, 3 bytes, into 4 tokens; pinned llama.cpp `77f132c`
/// tokenized it 10,000 times, 30,000 bytes, into 40,002 tokens, whose compact
/// answer of 210,016 bytes fits the 395,712 bytes allowed here, since real
/// IDs are shorter than the widest.
pub(super) const fn tokens_limit(text_bytes: usize) -> usize {
    text_bytes
        .saturating_add(ADDED_TOKENS)
        .saturating_mul(BYTES_PER_TOKEN)
        .saturating_add(ENVELOPE_BYTES)
}

/// Maximum `/v1/embeddings` body for `inputs` vectors of `dimensions`:
/// `inputs` × (`dimensions` × [`BYTES_PER_COMPONENT`] + [`BYTES_PER_ITEM`]) +
/// [`ENVELOPE_BYTES`].
pub(super) const fn embeddings_limit(inputs: usize, dimensions: usize) -> usize {
    dimensions
        .saturating_mul(BYTES_PER_COMPONENT)
        .saturating_add(BYTES_PER_ITEM)
        .saturating_mul(inputs)
        .saturating_add(ENVELOPE_BYTES)
}

/// Maximum `/v1/rerank` body for `documents` documents, whose texts the
/// answer does not repeat: `documents` × [`BYTES_PER_ITEM`] +
/// [`ENVELOPE_BYTES`].
pub(super) const fn ranking_limit(documents: usize) -> usize {
    documents
        .saturating_mul(BYTES_PER_ITEM)
        .saturating_add(ENVELOPE_BYTES)
}

/// Reads the body of `response`, refusing it once it would exceed `limit`
/// bytes: at once when it declares a longer length, otherwise at the chunk
/// that crosses the limit.
///
/// # Errors
///
/// [`Error::InvalidAnswer`] when the body exceeds `limit`, and
/// [`Error::Transport`] when the exchange breaks off.
pub(super) async fn read_bounded(response: &mut Response, limit: usize) -> Result<Vec<u8>, Error> {
    let over = || Error::InvalidAnswer {
        reason: format!("the answer exceeds its limit of {limit} bytes"),
    };
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(over());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(Error::Transport)? {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(over());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
