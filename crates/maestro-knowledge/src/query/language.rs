//! Language detection for normalized queries.

use whatlang::{Lang, detect};

/// The language understood for a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// French text confidently detected by whatlang.
    French,
    /// English text confidently detected by whatlang.
    English,
    /// Short, ambiguous, unsupported, or insufficiently confident text.
    Unknown,
}

/// Normalized queries shorter than 12 Unicode scalar values stay unknown.
const MINIMUM_LANGUAGE_CHARACTERS: usize = 12;
/// whatlang confidence required to assign a supported language.
const LANGUAGE_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Detects English or French, treating short or low-confidence input as
/// unknown.
pub(super) fn detect_language(text: &str) -> Language {
    if text.chars().count() < MINIMUM_LANGUAGE_CHARACTERS {
        return Language::Unknown;
    }
    let Some(info) = detect(text) else {
        return Language::Unknown;
    };
    if !is_confident(info.confidence()) {
        return Language::Unknown;
    }
    match info.lang() {
        Lang::Fra => Language::French,
        Lang::Eng => Language::English,
        _ => Language::Unknown,
    }
}

/// Whether whatlang's `confidence` reaches [`LANGUAGE_CONFIDENCE_THRESHOLD`].
fn is_confident(confidence: f64) -> bool {
    confidence >= LANGUAGE_CONFIDENCE_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::is_confident;

    #[test]
    fn a_confidence_of_exactly_one_half_is_confident_and_less_is_not() {
        assert!(is_confident(0.5));
        assert!(is_confident(1.0));
        assert!(!is_confident(0.499_999));
    }
}
