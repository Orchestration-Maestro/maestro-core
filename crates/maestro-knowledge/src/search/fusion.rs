//! Reciprocal rank fusion over independent retrieval routes.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

/// The reciprocal-rank-fusion constant in the denominator.
const RRF_K: f64 = 60.0;

/// A retrieval route contributing ranked chunks to fusion.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Route {
    /// Dense vector retrieval.
    Dense,
    /// Lexical retrieval.
    Lexical,
    /// Exact identifier retrieval.
    Identifier,
    /// Structured retrieval.
    Structured,
}

impl Route {
    /// Returns the stable trace name of this route.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dense => "dense",
            Self::Lexical => "lexical",
            Self::Identifier => "identifier",
            Self::Structured => "structured",
        }
    }
}

/// A route's hit, with its route-specific score retained for tracing.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// The stable identity of the retrieved chunk.
    pub chunk_id: String,
    /// The route's own score; it does not contribute to the fused score.
    pub score: f64,
}

/// A route's hits in the order that route ranked them.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteList {
    /// The route that produced these hits.
    pub route: Route,
    /// Hits in route rank order, highest-ranked first.
    pub hits: Vec<Hit>,
}

/// A chunk's fused score and one-based rank in each route that held it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fused {
    /// The stable identity of the chunk.
    pub chunk_id: String,
    /// The sum of its reciprocal one-based ranks.
    pub score: f64,
    /// Its one-based rank in each route that returned the chunk.
    pub ranks: BTreeMap<Route, NonZeroU32>,
}

/// Fuses route rankings with equal-weight reciprocal rank fusion and truncates
/// the result to `limit` chunks.
///
/// One list per route is expected; duplicate route lists are accepted as given
/// but trigger a debug assertion.
///
/// # Panics
///
/// Panics if a route has more than `u32::MAX` unique chunks, which cannot be
/// represented by the rank type.
#[must_use]
pub fn fuse(lists: &[RouteList], limit: usize) -> Vec<Fused> {
    debug_assert!(
        lists
            .iter()
            .map(|list| list.route)
            .collect::<BTreeSet<_>>()
            .len()
            == lists.len(),
        "one list per route is expected"
    );

    let mut ranks_by_chunk = BTreeMap::<String, BTreeMap<Route, NonZeroU32>>::new();
    for list in lists {
        let mut seen = BTreeSet::new();
        for hit in &list.hits {
            if !seen.insert(hit.chunk_id.as_str()) {
                continue;
            }
            // `NonZeroU32` cannot represent a larger route rank; fail instead
            // of assigning a false rank or silently dropping a chunk.
            #[expect(
                clippy::expect_used,
                reason = "rank overflow cannot be represented; fail rather than assign a false rank"
            )]
            let rank = NonZeroU32::new(
                u32::try_from(seen.len()).expect("route rank exceeds the supported u32 range"),
            )
            .expect("route ranks are one-based");
            ranks_by_chunk
                .entry(hit.chunk_id.clone())
                .or_default()
                .insert(list.route, rank);
        }
    }

    let mut fused = ranks_by_chunk
        .into_iter()
        .map(|(chunk_id, ranks)| {
            let score = ranks
                .values()
                .map(|rank| 1.0 / (RRF_K + f64::from(rank.get())))
                .sum();
            Fused {
                chunk_id,
                score,
                ranks,
            }
        })
        .collect::<Vec<_>>();
    fused.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    fused.truncate(limit);
    fused
}
