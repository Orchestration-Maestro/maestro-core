//! Stemming: light suffix rules on folded, lowercased words, French and
//! English together; the lexical module's documentation states each rule
//! with an example it merges and one it must not.

/// The stem of `word`, folded and lowercased: `word` itself when it holds a
/// digit, otherwise what the five steps leave, each applying the first of its
/// rules that fits.
pub(super) fn stem(word: &str) -> String {
    let mut stem = word.to_owned();
    if !word.chars().any(char::is_numeric) {
        plural(&mut stem);
        english_ending(&mut stem);
        french_ending(&mut stem);
        infinitive(&mut stem);
        final_y(&mut stem);
    }
    stem
}

/// Step 1: an `x` after `eau` or `eu` goes, `aux` becomes `al`, and a plural
/// `s` goes.
fn plural(word: &mut String) {
    if word.ends_with("eaux") || word.ends_with("eux") {
        word.pop();
    } else if let Some(stem) = word.strip_suffix("aux") {
        let length = stem.len();
        word.truncate(length);
        word.push_str("al");
    } else if ends_in_plural_s(word) {
        word.pop();
    }
}

/// Whether `word`'s last letter is a plural `s`: it does not end in `ss` or
/// `us`, and a vowel comes before the letter before the `s`.
fn ends_in_plural_s(word: &str) -> bool {
    let mut letters = word.chars().rev();
    match (letters.next(), letters.next()) {
        (Some('s'), Some(before)) => before != 's' && before != 'u' && letters.any(is_vowel),
        _ => false,
    }
}

/// Step 2: a final `ied` becomes `ie`, which step 3 turns into `i` when a stem
/// stays; else English `ed` or `ing` goes when a stem stays, and when that
/// leaves `eed`, its `ed` goes too when a stem stays, as from the bare verb.
fn english_ending(word: &mut String) {
    if word.ends_with("ied") {
        word.pop();
    } else if cut(word, "ed") || cut(word, "ing") {
        undouble(word);
        if word.ends_with("eed") {
            cut(word, "ed");
        }
    }
}

/// Step 3: French `ee` or `e`, feminine or a past participle's folded `é`,
/// goes when a stem stays.
fn french_ending(word: &mut String) {
    if cut(word, "ee") || cut(word, "e") {
        undouble(word);
    }
}

/// Step 4: an infinitive's `er` goes when a stem stays.
fn infinitive(word: &mut String) {
    if cut(word, "er") {
        undouble(word);
    }
}

/// Step 5: a final `y` after a consonant becomes `i`.
fn final_y(word: &mut String) {
    let mut letters = word.chars().rev();
    if matches!((letters.next(), letters.next()), (Some('y'), Some(before)) if !is_vowel(before)) {
        word.pop();
        word.push('i');
    }
}

/// Drops `ending` from `word` when what stays is a stem, and says whether it
/// did.
fn cut(word: &mut String, ending: &str) -> bool {
    match word.strip_suffix(ending) {
        Some(stem) if is_stem(stem) => {
            let length = stem.len();
            word.truncate(length);
            true
        }
        _ => false,
    }
}

/// Whether `stem` can be a stem: three letters or more, a vowel among them.
fn is_stem(stem: &str) -> bool {
    stem.chars().count() >= 3 && stem.chars().any(is_vowel)
}

/// Drops the last letter of a final `bb`, `dd`, `gg`, `mm`, `nn`, `pp`, `rr`
/// or `tt` when three letters stay.
fn undouble(word: &mut String) {
    let mut letters = word.chars().rev();
    let doubled = match (letters.next(), letters.next()) {
        (Some(last), Some(before)) => last == before && doubles(last),
        _ => false,
    };
    if doubled && word.chars().count() > 3 {
        word.pop();
    }
}

/// Whether `letter` is a consonant English or French doubles before an
/// ending; not `f`, since a final `ff` is the base's own (`stuff`, `staff`).
fn doubles(letter: char) -> bool {
    matches!(letter, 'b' | 'd' | 'g' | 'm' | 'n' | 'p' | 'r' | 't')
}

/// Whether `letter` is a vowel, `y` included.
fn is_vowel(letter: char) -> bool {
    matches!(letter, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}
