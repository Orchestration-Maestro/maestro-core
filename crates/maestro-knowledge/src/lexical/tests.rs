//! What the lexical module's lookups rely on.

use super::stopwords::{ENGLISH, FRENCH};

#[test]
fn stopword_lists_are_sorted_without_repeats() {
    for list in [&ENGLISH[..], &FRENCH[..]] {
        assert!(list.windows(2).all(|pair| pair[0] < pair[1]), "{list:?}");
    }
}

/// A word is looked up folded and lowercased, which for French and English
/// leaves only the letters `a` to `z`: a stopword written otherwise, such as
/// `été` or `Où`, would never match.
#[test]
fn stopword_lists_hold_only_folded_lowercase_letters() {
    for word in ENGLISH.iter().chain(&FRENCH) {
        assert!(!word.is_empty(), "an empty stopword");
        assert!(
            word.bytes().all(|byte| byte.is_ascii_lowercase()),
            "{word:?}"
        );
    }
}
