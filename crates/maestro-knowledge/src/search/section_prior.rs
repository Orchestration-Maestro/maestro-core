//! Configured section classes and an optional soft reciprocal-rank penalty.

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

    /// The reciprocal-rank multiplier of a penalized candidate; the shared
    /// demotion moves a penalized rank r to about r / (1 - weight).
    pub(super) fn multiplier(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Soft { weight, .. } => Some(1.0 - f64::from(weight)),
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
