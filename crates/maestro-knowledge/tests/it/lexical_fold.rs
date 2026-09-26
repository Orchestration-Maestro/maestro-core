//! Folding in `bm25-en-fr/1`: every letter of Latin-1 Supplement and Latin
//! Extended-A that has a canonical decomposition gives the same terms
//! precomposed (NFC), decomposed (NFD) and without its mark, and the
//! typographic ligatures of PDF text give the terms of their letters.
#![cfg(test)]

use maestro_knowledge::lexical::terms;

/// Each combining mark, the precomposed letters of Latin-1 Supplement and
/// Latin Extended-A that decompose into a base letter and that mark, and
/// their base letters in the same order: the 161 letters of the two blocks
/// that Unicode's data gives a canonical decomposition.
const DECOMPOSED: [(char, &str, &str); 13] = [
    ('\u{300}', "ÀÈÌÒÙàèìòù", "AEIOUaeiou"),
    (
        '\u{301}',
        "ÁÉÍÓÚÝáéíóúýĆćĹĺŃńŔŕŚśŹź",
        "AEIOUYaeiouyCcLlNnRrSsZz",
    ),
    (
        '\u{302}',
        "ÂÊÎÔÛâêîôûĈĉĜĝĤĥĴĵŜŝŴŵŶŷ",
        "AEIOUaeiouCcGgHhJjSsWwYy",
    ),
    ('\u{303}', "ÃÑÕãñõĨĩŨũ", "ANOanoIiUu"),
    ('\u{304}', "ĀāĒēĪīŌōŪū", "AaEeIiOoUu"),
    ('\u{306}', "ĂăĔĕĞğĬĭŎŏŬŭ", "AaEeGgIiOoUu"),
    ('\u{307}', "ĊċĖėĠġİŻż", "CcEeGgIZz"),
    ('\u{308}', "ÄËÏÖÜäëïöüÿŸ", "AEIOUaeiouyY"),
    ('\u{30a}', "ÅåŮů", "AaUu"),
    ('\u{30b}', "ŐőŰű", "OoUu"),
    ('\u{30c}', "ČčĎďĚěĽľŇňŘřŠšŤťŽž", "CcDdEeLlNnRrSsTtZz"),
    ('\u{327}', "ÇçĢģĶķĻļŅņŖŗŞşŢţ", "CcGgKkLlNnRrSsTt"),
    ('\u{328}', "ĄąĘęĮįŲų", "AaEeIiUu"),
];

#[test]
fn precomposed_and_decomposed_letters_give_the_same_terms() {
    let mut letters = 0;
    for (mark, precomposed, bases) in DECOMPOSED {
        assert_eq!(
            precomposed.chars().count(),
            bases.chars().count(),
            "{precomposed}"
        );
        for (letter, base) in precomposed.chars().zip(bases.chars()) {
            let plain = terms(&format!("x{base}z"));
            assert_eq!(terms(&format!("x{letter}z")), plain, "{letter}");
            assert_eq!(terms(&format!("x{base}{mark}z")), plain, "{letter}");
            letters += 1;
        }
    }
    assert_eq!(letters, 161);
}

#[test]
fn typographic_ligatures_fold_into_their_letters() {
    assert_eq!(
        terms("ﬁle ﬂow eﬀort diﬃcult baﬄe"),
        ["fil", "flow", "effort", "difficult", "baffl"]
    );
}
