//! The BCP 47 subset of `language`: canonical case, and each refused form
//! named.

use crate::canonical_language;

#[test]
fn canonical_language_accepts_the_subset_in_canonical_case() {
    for (text, canonical) in [
        ("en", "en"),
        ("FR", "fr"),
        ("ja", "ja"),
        ("es-419", "es-419"),
        ("sr-latn", "sr-Latn"),
        ("zh-hant-tw", "zh-Hant-TW"),
        ("ZH-HANT-TW", "zh-Hant-TW"),
        ("fr-ca", "fr-CA"),
        ("yue", "yue"),
    ] {
        assert_eq!(canonical_language(text).as_deref(), Ok(canonical), "{text}");
    }
}

#[test]
fn canonical_language_refuses_every_form_outside_the_subset() {
    for (text, reason) in [
        ("", "a language tag starts with a 2 or 3 letter language"),
        (
            "e",
            "private-use and irregular legacy tags are not supported",
        ),
        (
            "x-private",
            "private-use and irregular legacy tags are not supported",
        ),
        (
            "i-klingon",
            "private-use and irregular legacy tags are not supported",
        ),
        (
            "english",
            "a language tag starts with a 2 or 3 letter language",
        ),
        ("e1", "a language tag starts with a 2 or 3 letter language"),
        ("français", "a language tag is ASCII"),
        ("en-", "a language tag has an empty subtag"),
        ("en--US", "a language tag has an empty subtag"),
        (
            "de-DE-1996",
            "only a language, a script and a region are supported: \
                        variants, extensions and private use are not",
        ),
        (
            "en-a-bbb",
            "only a language, a script and a region are supported: \
                      variants, extensions and private use are not",
        ),
        (
            "en-US-x-foo",
            "only a language, a script and a region are supported: \
                         variants, extensions and private use are not",
        ),
        (
            "en-12",
            "only a language, a script and a region are supported: \
                   variants, extensions and private use are not",
        ),
        (
            "en-Latn-Latn",
            "only a language, a script and a region are supported: \
                          variants, extensions and private use are not",
        ),
        (
            "en-US-CA",
            "only a language, a script and a region are supported: \
                      variants, extensions and private use are not",
        ),
    ] {
        assert_eq!(canonical_language(text), Err(reason), "{text:?}");
    }
}
