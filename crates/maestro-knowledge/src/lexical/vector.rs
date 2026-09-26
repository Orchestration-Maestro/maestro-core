//! Sparse vectors: a passage's terms weighed with BM25's term-frequency part
//! and a query's with 1, each under a 32-bit token ID.

use super::analyzer::terms;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// BM25's k1: how fast a term's weight saturates as the term repeats.
const SATURATION: f64 = 1.2;

/// BM25's b: how much a passage longer than the average lowers its weights.
const NORMALIZATION: f64 = 0.75;

/// The average term count of a generation's passages: a positive, finite
/// number, which the caller computes over every passage first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AverageLength(f64);

impl AverageLength {
    /// `terms` as an average length, or nothing when it is not a positive,
    /// finite number.
    #[must_use]
    pub fn new(terms: f64) -> Option<Self> {
        (terms.is_finite() && terms > 0.0).then_some(Self(terms))
    }
}

/// A passage's terms, counted: what its sparse vector is weighed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passage {
    /// How often each term occurs.
    frequencies: BTreeMap<String, usize>,
    /// How many terms the passage holds, repeats included.
    length: usize,
}

impl Passage {
    /// The terms of the passage `text`, counted.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let terms = terms(text);
        let length = terms.len();
        let mut frequencies = BTreeMap::new();
        for term in terms {
            *frequencies.entry(term).or_insert(0) += 1;
        }
        Self {
            frequencies,
            length,
        }
    }

    /// How many terms the passage holds, repeats included: the length BM25
    /// sets against the average.
    #[must_use]
    pub fn term_count(&self) -> usize {
        self.length
    }

    /// The passage's vector: a term occurring `tf` times weighs
    /// `tf·(k1+1) / (tf + k1·(1 − b + b·len/avg_len))`, with k1 1.2, b 0.75,
    /// `len` the passage's term count and `avg_len` the generation's
    /// `average`. Qdrant multiplies it by the term's IDF.
    #[must_use]
    pub fn vector(&self, average: AverageLength) -> SparseVector {
        let norm =
            SATURATION * (1.0 - NORMALIZATION + NORMALIZATION * float(self.length) / average.0);
        SparseVector::summed(self.frequencies.iter().map(|(term, frequency)| {
            let frequency = float(*frequency);
            (
                token_id(term),
                frequency * (SATURATION + 1.0) / (frequency + norm),
            )
        }))
    }
}

/// The vector of the query `text`: each of its terms weighs 1, however often
/// it occurs.
#[must_use]
pub fn query_vector(text: &str) -> SparseVector {
    let terms: BTreeSet<String> = terms(text).into_iter().collect();
    SparseVector::summed(terms.iter().map(|term| (token_id(term), 1.0)))
}

/// A sparse vector as Qdrant takes it: token IDs, sorted and unique, and the
/// weight of each. The same text always gives the same vector, bit for bit.
#[derive(Debug, Clone, PartialEq)]
pub struct SparseVector {
    /// The token IDs, sorted and unique.
    indices: Vec<u32>,
    /// The weight of each token ID.
    values: Vec<f32>,
}

impl SparseVector {
    /// The vector of `weights`, a token ID's weights added: two terms that
    /// share an ID count as one.
    fn summed(weights: impl Iterator<Item = (u32, f64)>) -> Self {
        let mut summed = BTreeMap::new();
        for (index, weight) in weights {
            *summed.entry(index).or_insert(0.0) += weight;
        }
        let (indices, values) = summed
            .into_iter()
            .map(|(index, weight)| (index, narrow(weight)))
            .unzip();
        Self { indices, values }
    }

    /// The token IDs, sorted and unique.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// The weight of each token ID, in the order of [`Self::indices`].
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

/// A term's token ID: the first four bytes of its SHA-256, big-endian.
fn token_id(term: &str) -> u32 {
    let [first, second, third, fourth, ..] = Sha256::digest(term.as_bytes()).0;
    u32::from_be_bytes([first, second, third, fourth])
}

/// `count` as a float, exactly: no passage holds 2^53 terms.
#[expect(
    clippy::cast_precision_loss,
    reason = "a term count stays far below 2^53, where f64 is exact"
)]
fn float(count: usize) -> f64 {
    count as f64
}

/// `weight` as Qdrant stores it, rounded to the nearest `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "Qdrant stores a sparse vector's weights as f32"
)]
fn narrow(weight: f64) -> f32 {
    weight as f32
}
