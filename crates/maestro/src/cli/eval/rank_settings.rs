//! Manifest adapters for optional candidate-context and section-prior policies.

use crate::failure::Failure;
use maestro_knowledge::search::{CandidateContext, SectionClassSet, SectionPrior};
use serde::{Deserialize, Serialize};

/// Reranker input context; absent settings retain indexed chunks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Context {
    /// Indexed prepared input.
    #[default]
    Chunk,
    /// Whole source units, with a strict expansion budget.
    BoundedSection {
        /// Maximum expanded bytes, up to 1500.
        max_bytes: usize,
    },
}

impl Context {
    /// Converts the manifest shape to the public search policy.
    pub(super) const fn search(self) -> CandidateContext {
        match self {
            Self::Chunk => CandidateContext::Chunk,
            Self::BoundedSection { max_bytes } => CandidateContext::BoundedSection { max_bytes },
        }
    }

    /// Refuses invalid expansion bounds before running any rung.
    pub(super) fn check(self) -> Result<(), Failure> {
        if let Self::BoundedSection { max_bytes } = self
            && !(1..=1500).contains(&max_bytes)
        {
            return Err(Failure::refused(
                "candidate context max_bytes must be between 1 and 1500",
            ));
        }
        Ok(())
    }
}

/// Named-class soft section penalty; disabled unless explicitly configured.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Prior {
    /// No section preference.
    #[default]
    Off,
    /// Reciprocal-rank penalty for matching, non-exempt sections.
    Soft {
        /// Fraction removed, in 0..=1.
        weight: f32,
        /// Stable class names, validated against the shared class vocabulary.
        classes: Vec<String>,
    },
}

impl Prior {
    /// Validates names and weight, returning the Copy search configuration.
    pub(super) fn search(&self) -> Result<SectionPrior, Failure> {
        let Self::Soft { weight, classes } = self else {
            return Ok(SectionPrior::Off);
        };
        let mut selected = SectionClassSet::default();
        for name in classes {
            if !selected.insert(name) {
                return Err(Failure::refused("unknown section prior class"));
            }
        }
        let prior = SectionPrior::Soft {
            weight: *weight,
            classes: selected,
        };
        if !prior.is_valid() {
            return Err(Failure::refused(
                "section prior weight must be between 0 and 1",
            ));
        }
        Ok(prior)
    }
}
