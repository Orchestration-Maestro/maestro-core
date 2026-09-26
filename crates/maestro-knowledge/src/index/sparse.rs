//! Sparse vectors: each prepared input's terms weighed against the average
//! term count of every chunk of its chunk set, counted in a first pass.

use crate::lexical::{AverageLength, Passage, SparseVector};

/// The terms of a chunk set's prepared inputs, as the analyzer `bm25-en-fr/1`
/// counts them, summed over a first pass: what BM25's average passage length
/// comes from, before any vector is weighed. Every chunk counts, one with no
/// term too.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Lengths {
    /// The terms counted, repeats included.
    terms: u64,
    /// The passages counted.
    passages: u64,
}

impl Lengths {
    /// Counts the terms of the prepared input `text`, one passage more.
    pub(super) fn add(&mut self, text: &str) {
        let terms = u64::try_from(Passage::new(text).term_count()).unwrap_or(u64::MAX);
        self.terms = self.terms.saturating_add(terms);
        self.passages += 1;
    }

    /// The average term count of the passages counted; zero when none holds
    /// a term, or none was counted.
    pub(super) fn mean(&self) -> f64 {
        if self.passages == 0 {
            0.0
        } else {
            float(self.terms) / float(self.passages)
        }
    }

    /// The sparse vector of the prepared input `text`, its terms weighed
    /// against [`Lengths::mean`]; none when no passage counted holds a term,
    /// since every vector is empty then.
    pub(super) fn vector(&self, text: &str) -> Option<SparseVector> {
        AverageLength::new(self.mean()).map(|average| Passage::new(text).vector(average))
    }
}

/// `count` as a float: a chunk set counts far fewer than 2^53 terms, where
/// `f64` stays exact.
#[expect(
    clippy::cast_precision_loss,
    reason = "a chunk set counts far fewer than 2^53 terms, where f64 is exact"
)]
fn float(count: u64) -> f64 {
    count as f64
}
