//! Folding: a text without its accents, before any other rule reads it.

/// `text` without the combining marks U+0300 to U+036F, and with each
/// precomposed letter of Latin-1 Supplement and Latin Extended-A written as
/// its base letter, case kept: the 161 letters of the two blocks that have a
/// canonical decomposition, which is always a base letter and one of those
/// marks, so a letter folds alike precomposed (NFC) and decomposed (NFD). `é`
/// and `É` become `e` and `E`, and `ñ`, like `n` followed by U+0303, becomes
/// `n`. The ligatures `æ` and `œ`, and the typographic ligatures `ﬀ`, `ﬁ`,
/// `ﬂ`, `ﬃ` and `ﬄ` of text taken from PDF files, are written as their
/// letters: `Œ` becomes `OE` and `ﬁ` becomes `fi`. A letter outside the two
/// blocks, such as Vietnamese `ệ`, folds only when decomposed.
pub(super) fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for character in text.chars() {
        match base_letters(character) {
            Some(letters) => folded.push_str(letters),
            None if is_combining_mark(character) => {}
            None => folded.push(character),
        }
    }
    folded
}

/// The base letters of `character`, when it is a precomposed letter of
/// Latin-1 Supplement or Latin Extended-A, or a ligature.
fn base_letters(character: char) -> Option<&'static str> {
    Some(match character {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => "o",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ţ' | 'ť' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        'œ' => "oe",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'Ď' => "D",
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
        'Ĥ' => "H",
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
        'Ĵ' => "J",
        'Ķ' => "K",
        'Ĺ' | 'Ļ' | 'Ľ' => "L",
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ō' | 'Ŏ' | 'Ő' => "O",
        'Ŕ' | 'Ŗ' | 'Ř' => "R",
        'Ś' | 'Ŝ' | 'Ş' | 'Š' => "S",
        'Ţ' | 'Ť' => "T",
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'Ŵ' => "W",
        'Ý' | 'Ŷ' | 'Ÿ' => "Y",
        'Ź' | 'Ż' | 'Ž' => "Z",
        'Æ' => "AE",
        'Œ' => "OE",
        'ﬀ' => "ff",
        'ﬁ' => "fi",
        'ﬂ' => "fl",
        'ﬃ' => "ffi",
        'ﬄ' => "ffl",
        _ => return None,
    })
}

/// Whether `character` is a combining mark of the block that holds the
/// accents of decomposed text, U+0300 to U+036F.
fn is_combining_mark(character: char) -> bool {
    matches!(character, '\u{300}'..='\u{36f}')
}
