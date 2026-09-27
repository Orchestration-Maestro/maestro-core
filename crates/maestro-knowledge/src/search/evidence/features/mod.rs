//! Case-sensitive shingles and protected source signatures.

#[cfg(test)]
mod mutation_tests;
mod signatures;

pub(super) use signatures::has_condition_or_negation;
#[cfg(test)]
pub(super) use signatures::{
    DiversityFeatures, diversity_features, diversity_similarity, mmr_score, word_shingles,
};
