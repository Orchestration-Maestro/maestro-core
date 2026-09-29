//! Per-language word lists of the expansion guard: data, not rules.

/// One language's constraint vocabulary, all lower case.
pub(super) struct Lexicon {
    /// Articles, skipped when phrases are compared ("after the third" is
    /// "after third"). French `un` and `une` are articles here, not one.
    pub(super) articles: &'static [&'static str],
    /// Words that bind the next word: the pair must survive together, as
    /// in "from a step" or "only after".
    pub(super) relations: &'static [&'static str],
    /// Negations and other constraint words that must survive alone.
    pub(super) constraints: &'static [&'static str],
    /// Number words and ordinals, with the quantity each one names.
    pub(super) quantities: &'static [(&'static str, u32)],
    /// Ordinal endings after a numeral, as in "3rd" or "3e".
    pub(super) ordinal_endings: &'static [&'static str],
}

/// The languages whose constraint words the guard knows.
pub(super) const LEXICONS: &[Lexicon] = &[ENGLISH, FRENCH];

/// English.
const ENGLISH: Lexicon = Lexicon {
    articles: &["a", "an", "the"],
    relations: &["from", "after", "before", "without", "only", "until"],
    constraints: &[
        "no", "not", "never", "except", "unless", "at", "least", "most", "don't", "cannot", "can't",
    ],
    quantities: &[
        ("zero", 0),
        ("one", 1),
        ("first", 1),
        ("two", 2),
        ("second", 2),
        ("three", 3),
        ("third", 3),
        ("four", 4),
        ("fourth", 4),
        ("five", 5),
        ("fifth", 5),
        ("six", 6),
        ("sixth", 6),
        ("seven", 7),
        ("seventh", 7),
        ("eight", 8),
        ("eighth", 8),
        ("nine", 9),
        ("ninth", 9),
        ("ten", 10),
        ("tenth", 10),
    ],
    ordinal_endings: &["st", "nd", "rd", "th"],
};

/// French.
const FRENCH: Lexicon = Lexicon {
    articles: &["le", "la", "les", "l", "un", "une", "des"],
    relations: &[
        "sans",
        "avant",
        "après",
        "depuis",
        "seulement",
        "uniquement",
        "jusqu'à",
    ],
    constraints: &["pas", "jamais", "aucun", "aucune", "sauf", "ni"],
    quantities: &[
        ("zéro", 0),
        ("premier", 1),
        ("première", 1),
        ("deux", 2),
        ("deuxième", 2),
        ("seconde", 2),
        ("trois", 3),
        ("troisième", 3),
        ("quatre", 4),
        ("quatrième", 4),
        ("cinq", 5),
        ("cinquième", 5),
        ("sixième", 6),
        ("sept", 7),
        ("septième", 7),
        ("huit", 8),
        ("huitième", 8),
        ("neuf", 9),
        ("neuvième", 9),
        ("dix", 10),
        ("dixième", 10),
    ],
    ordinal_endings: &["e", "er", "re", "ème", "eme", "ième"],
};
