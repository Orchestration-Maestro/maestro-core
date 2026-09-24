//! The pure preparation helpers: cut points, fitting prefixes and delimiter-safe ranges.
use super::super::prepare::{boundaries, normalized_range};
use super::*;
use crate::chunks::{InlineEnvelope, TextRange};

#[test]
fn cut_points_follow_whitespace_and_meaningful_ones_end_sentences_or_code_lines() {
    let text = "One. Two\nthree  four";
    assert_eq!(boundaries(text, false), (vec![5], vec![5, 9, 15, 16]));
    assert_eq!(boundaries(text, true), (vec![9], vec![5, 9, 15, 16]));
    // Offsets are in bytes, after the whitespace: `。` is a sentence end too.
    assert_eq!(boundaries("終わり。 次", false), (vec![13], vec![13]));
}

#[test]
fn a_fitting_prefix_ends_at_the_last_preferred_cut_inside_it() {
    let mut fits = |_: usize| Ok(true);
    assert_eq!(fit_prefix("ab cd", &[3, 5], &mut fits).unwrap(), 3);
    // A cut at the start, past the end or inside a character is never taken.
    assert_eq!(fit_prefix("ab cd", &[0], &mut fits).unwrap(), 5);
    assert_eq!(fit_prefix("ab cd", &[9], &mut fits).unwrap(), 5);
    assert_eq!(fit_prefix("éa b", &[1], &mut fits).unwrap(), 5);
    assert!(fit_prefix("", &[], &mut fits).is_err());
}

#[test]
fn ranges_never_split_a_delimiter_and_take_in_a_closing_one() {
    let markdown = "text\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "envelopes")).unwrap();
    let mut unit = map_document(&doc, markdown).unwrap().units.remove(0);
    // Only the text and its envelopes matter: `~~b~~` inside other text.
    unit.text = "a ~~b~~ c".into();
    unit.envelopes = vec![InlineEnvelope {
        opening: TextRange { start: 2, end: 4 },
        closing: TextRange { start: 5, end: 7 },
    }];
    let range = |start, end| normalized_range(&unit, start, end).map(|r| (r.start, r.end));
    // Starting at an opening delimiter, or ending where one starts, splits nothing.
    assert_eq!(range(2, 9), Some((2, 9)));
    assert_eq!(range(0, 2), Some((0, 2)));
    assert_eq!(range(0, 3), None);
    assert_eq!(range(0, 6), Some((0, 7)));
    assert_eq!(range(4, 5), Some((4, 7)));
    // A closing delimiter alone holds no text.
    assert_eq!(range(5, 7), None);
}
