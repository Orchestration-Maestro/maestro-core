//! Configured section classes and an optional soft reciprocal-rank penalty.

use super::rerank::Ranked;
use std::collections::BTreeSet;

/// Enabled named section classes; unknown names cannot enter the set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionClassSet(u8);

/// Classification data, not page identities or question classifiers.
struct Class {
    /// Stable manifest spelling.
    name: &'static str,
    /// Heading/path phrases identifying the section.
    terms: &'static [&'static str],
    /// Explicit English and French phrases naming the class; a question that
    /// uses one is about these sections, so the prior stays off.
    exemptions: &'static [&'static str],
}

/// The class vocabulary is data so callers never branch on class names.
const CLASSES: &[Class] = &[
    Class {
        name: "changelog",
        terms: &["changelog", "change log"],
        exemptions: &[
            "changelog",
            "change log",
            "journal des modifications",
            "historique des modifications",
        ],
    },
    Class {
        name: "release_notes",
        terms: &["release notes", "release note"],
        exemptions: &[
            "release notes",
            "release note",
            "notes de version",
            "note de version",
        ],
    },
    Class {
        name: "conversion",
        terms: &["conversion"],
        exemptions: &["conversion", "convert", "converting", "convertir"],
    },
];

impl SectionClassSet {
    /// Adds a known class, returning false without modification for an unknown name.
    pub fn insert(&mut self, name: &str) -> bool {
        let Some(index) = CLASSES.iter().position(|class| class.name == name) else {
            return false;
        };
        self.0 |= 1 << index;
        true
    }
}

/// A soft section prior, disabled by default.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SectionPrior {
    /// Preserve today's order exactly.
    #[default]
    Off,
    /// Down-weight matching sections unless the query explicitly names their class.
    Soft {
        /// Fraction of reciprocal-rank score removed, in 0..=1.
        weight: f32,
        /// Configured section classes to consider.
        classes: SectionClassSet,
    },
}

impl SectionPrior {
    /// Whether all public numeric settings are valid.
    #[must_use]
    pub fn is_valid(self) -> bool {
        match self {
            Self::Off => true,
            Self::Soft { weight, .. } => weight.is_finite() && (0.0..=1.0).contains(&weight),
        }
    }

    /// Classifies only heading/path metadata; body text is deliberately excluded.
    /// A question naming any configured class switches the prior off.
    pub(super) fn penalizes(self, query: &str, path: &str) -> bool {
        let Self::Soft { classes, .. } = self else {
            return false;
        };
        let query = words(query);
        let path = words(path);
        let selected = || {
            CLASSES
                .iter()
                .enumerate()
                .filter(move |(index, _)| classes.0 & (1 << index) != 0)
                .map(|(_, class)| class)
        };
        !selected().any(|class| class.exemptions.iter().any(|term| phrase(&query, term)))
            && selected().any(|class| class.terms.iter().any(|term| phrase(&path, term)))
    }

    /// Whether the prior can penalize anything, so sources need loading.
    pub(super) const fn is_active(self) -> bool {
        matches!(self, Self::Soft { classes, .. } if classes.0 != 0)
    }

    /// Applies the configured penalty without changing any stored raw score.
    /// Each item scores `multiplier / (position + 1)`, so a penalized item at
    /// rank r moves to about r / (1 - weight); on equal scores the
    /// unpenalized item goes first, so any positive weight demotes strictly.
    #[expect(
        clippy::cast_precision_loss,
        reason = "search bounds the candidate pool to 120"
    )]
    pub(super) fn apply(self, ranked: &mut [Ranked], penalized: &BTreeSet<String>) {
        let Self::Soft { weight, .. } = self else {
            return;
        };
        let mut ordered = ranked
            .iter()
            .enumerate()
            .map(|(position, item)| {
                let demoted = penalized.contains(&item.candidate.fused.chunk_id);
                let multiplier = if demoted {
                    1.0 - f64::from(weight)
                } else {
                    1.0
                };
                (item.clone(), multiplier / (position + 1) as f64, demoted)
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.2.cmp(&right.2)));
        for (target, (item, _, _)) in ranked.iter_mut().zip(ordered) {
            *target = item;
        }
    }
}

/// Normalizes separators while retaining whole-word boundaries.
fn words(text: &str) -> String {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Matches a complete normalized phrase, never a substring inside another word.
fn phrase(text: &str, term: &str) -> bool {
    format!(" {text} ").contains(&format!(" {term} "))
}
