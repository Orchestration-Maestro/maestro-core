//! Folding: a text without its accents, before any other rule reads it.

/// `text` without the combining marks U+0300 to U+036F, and with the accented
/// letters and ligatures of French and English written in their base
/// letters, case kept: `é` and `É` become `e` and `E`, `œ` and `Œ` become
/// `oe` and `OE`.
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

/// The base letters of `character`, when it is an accented letter or a
/// ligature French or English writes.
fn base_letters(character: char) -> Option<&'static str> {
    Some(match character {
        'à' | 'â' | 'ä' => "a",
        'ç' => "c",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'î' | 'ï' => "i",
        'ô' | 'ö' => "o",
        'ù' | 'û' | 'ü' => "u",
        'ÿ' => "y",
        'æ' => "ae",
        'œ' => "oe",
        'À' | 'Â' | 'Ä' => "A",
        'Ç' => "C",
        'È' | 'É' | 'Ê' | 'Ë' => "E",
        'Î' | 'Ï' => "I",
        'Ô' | 'Ö' => "O",
        'Ù' | 'Û' | 'Ü' => "U",
        'Ÿ' => "Y",
        'Æ' => "AE",
        'Œ' => "OE",
        _ => return None,
    })
}

/// Whether `character` is a combining mark of the block that holds the
/// accents of decomposed text, U+0300 to U+036F.
fn is_combining_mark(character: char) -> bool {
    matches!(character, '\u{300}'..='\u{36f}')
}
