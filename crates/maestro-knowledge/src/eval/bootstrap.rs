//! The bootstrap: resamples of a run's questions, drawn within the
//! answerable and the unanswerable questions apart by a seeded generator, and
//! the percentile intervals of a statistic over them.

use super::report::Estimate;
use std::collections::BTreeMap;

/// How many resamples an interval is drawn from.
const RESAMPLES: usize = 2000;

/// A `SplitMix64` generator (Steele, Lea and Flood, 2014): the same seed
/// gives the same numbers on every platform.
#[derive(Debug)]
pub(super) struct Generator(u64);

impl Generator {
    /// The generator `seed` starts.
    pub(super) fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Its next number.
    pub(super) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    /// Its next index below `bound`: the high word of its next number times
    /// the bound, so each index is as likely as another but for one part in
    /// 2^64 / `bound`.
    pub(super) fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).unwrap_or(u64::MAX);
        let scaled = (u128::from(self.next_u64()) * u128::from(bound)) >> 64;
        usize::try_from(scaled).unwrap_or(usize::MAX)
    }
}

/// The indices of the items `answerable` says are, then of the others.
pub(super) fn strata(answerable: impl Iterator<Item = bool>) -> [Vec<usize>; 2] {
    let mut strata = [Vec::new(), Vec::new()];
    for (index, answerable) in answerable.enumerate() {
        let [answered, unanswered] = &mut strata;
        if answerable {
            answered.push(index);
        } else {
            unanswered.push(index);
        }
    }
    strata
}

/// A resample of the items of `strata`: from each stratum, as many items as
/// it holds, drawn with replacement by `generator`.
pub(super) fn resample(strata: &[Vec<usize>], generator: &mut Generator) -> Vec<usize> {
    let mut sample = Vec::new();
    for stratum in strata {
        for _ in stratum {
            sample.extend(stratum.get(generator.below(stratum.len())));
        }
    }
    sample
}

/// The value at `per_mille` of `sorted` by nearest rank: the smallest value
/// with at least that share of the values at or below it, if any.
pub(super) fn percentile<T: Copy>(sorted: &[T], per_mille: usize) -> Option<T> {
    let rank = per_mille.checked_mul(sorted.len())?.div_ceil(1000);
    sorted.get(rank.checked_sub(1)?).copied()
}

/// The estimate of each value `statistic` gives over every item, by its key:
/// that value, and the 2.5th and 97.5th percentiles of the values it gives
/// over [`RESAMPLES`] resamples, each drawing as many answerable and
/// unanswerable items as there are, which `answerable` tells apart, with the
/// generator `seed` starts. A resample that holds none of the items a value
/// covers, such as the searches that were not degraded, gives none: the
/// percentiles are those of the resamples that give one. `statistic` reads
/// the items at the indices it is given, which may repeat.
pub(super) fn estimates<K: Ord>(
    answerable: &[bool],
    seed: u64,
    statistic: impl Fn(&[usize]) -> BTreeMap<K, f64>,
) -> BTreeMap<K, Estimate> {
    let every: Vec<usize> = (0..answerable.len()).collect();
    let values = statistic(&every);
    let strata = strata(answerable.iter().copied());
    let mut generator = Generator::new(seed);
    let mut draws: BTreeMap<K, Vec<f64>> = BTreeMap::new();
    for _ in 0..RESAMPLES {
        for (metric, value) in statistic(&resample(&strata, &mut generator)) {
            draws.entry(metric).or_default().push(value);
        }
    }
    values
        .into_iter()
        .filter_map(|(metric, value)| {
            let mut drawn = draws.remove(&metric)?;
            drawn.sort_by(f64::total_cmp);
            let low = percentile(&drawn, 25)?;
            let high = percentile(&drawn, 975)?;
            Some((metric, Estimate { value, low, high }))
        })
        .collect()
}
