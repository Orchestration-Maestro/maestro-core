//! What the lexical module's lookups rely on.

use super::stopwords::{ENGLISH, FRENCH};

#[test]
fn stopword_lists_are_sorted_without_repeats() {
    for list in [&ENGLISH[..], &FRENCH[..]] {
        assert!(list.windows(2).all(|pair| pair[0] < pair[1]), "{list:?}");
    }
}
