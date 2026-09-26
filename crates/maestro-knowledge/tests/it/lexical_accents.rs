//! Properties of `bm25-en-fr/1` over generated texts: a text and the same
//! text without its accents give the same terms and the same vectors, and
//! analyzing a text twice gives the same vector, bit for bit.
#![cfg(test)]

use maestro_knowledge::lexical::{AverageLength, Passage, SparseVector, query_vector, terms};

/// The accented letters of French and English, and each written without its
/// accent: the letters `bm25-en-fr/1` folds.
const ACCENTED: [(char, &str); 36] = [
    ('à', "a"),
    ('â', "a"),
    ('ä', "a"),
    ('ç', "c"),
    ('è', "e"),
    ('é', "e"),
    ('ê', "e"),
    ('ë', "e"),
    ('î', "i"),
    ('ï', "i"),
    ('ô', "o"),
    ('ö', "o"),
    ('ù', "u"),
    ('û', "u"),
    ('ü', "u"),
    ('ÿ', "y"),
    ('æ', "ae"),
    ('œ', "oe"),
    ('À', "A"),
    ('Â', "A"),
    ('Ä', "A"),
    ('Ç', "C"),
    ('È', "E"),
    ('É', "E"),
    ('Ê', "E"),
    ('Ë', "E"),
    ('Î', "I"),
    ('Ï', "I"),
    ('Ô', "O"),
    ('Ö', "O"),
    ('Ù', "U"),
    ('Û', "U"),
    ('Ü', "U"),
    ('Ÿ', "Y"),
    ('Æ', "AE"),
    ('Œ', "OE"),
];

/// Combining marks, the first and the last of their block among them.
const MARKS: [char; 6] = [
    '\u{300}', '\u{301}', '\u{302}', '\u{308}', '\u{327}', '\u{36f}',
];

/// Letters and digits without accents, in both cases.
const PLAIN: &str = "aeiouybcdfglmnprstxzAEIOUYBCDLMNPRST0123456789";

/// What separates or joins words.
const BETWEEN: [&str; 11] = [" ", "'", "’", "-", "_", ".", "/", ":", "(", ")", ", "];

/// A small generator with a fixed seed, so that a failure replays.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % 1_000_003).unwrap()
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.next() % items.len()]
    }
}

/// A text with accented letters, and the same text without its accents.
fn texts(draws: &mut Draws) -> (String, String) {
    let plain: Vec<char> = PLAIN.chars().collect();
    let (mut accented, mut unaccented) = (String::new(), String::new());
    for _ in 0..=draws.next() % 60 {
        match draws.next() % 4 {
            0 => {
                let (letter, without) = draws.pick(&ACCENTED);
                accented.push(letter);
                unaccented.push_str(without);
            }
            1 => {
                let letter = draws.pick(&plain);
                accented.push(letter);
                accented.push(draws.pick(&MARKS));
                unaccented.push(letter);
            }
            2 => {
                let letter = draws.pick(&plain);
                accented.push(letter);
                unaccented.push(letter);
            }
            _ => {
                let between = draws.pick(&BETWEEN);
                accented.push_str(between);
                unaccented.push_str(between);
            }
        }
    }
    (accented, unaccented)
}

/// `vector`'s indices and the bits of its weights.
fn bits(vector: &SparseVector) -> (Vec<u32>, Vec<u32>) {
    let weights = vector.values().iter().map(|weight| weight.to_bits());
    (vector.indices().to_vec(), weights.collect())
}

#[test]
fn a_text_without_its_accents_gives_the_same_vectors() {
    let average = AverageLength::new(7.5).unwrap();
    let mut draws = Draws(0x9e37_79b9_7f4a_7c15);
    let mut with_terms = 0;
    for _ in 0..2000 {
        let (accented, unaccented) = texts(&mut draws);
        let found = terms(&accented);
        assert_eq!(found, terms(&unaccented), "{accented:?}, {unaccented:?}");
        with_terms += usize::from(!found.is_empty());
        let passages = [Passage::new(&accented), Passage::new(&unaccented)];
        let [with, without] = passages.map(|passage| bits(&passage.vector(average)));
        assert_eq!(with, without, "{accented:?}, {unaccented:?}");
        let queries = [query_vector(&accented), query_vector(&unaccented)];
        assert_eq!(bits(&queries[0]), bits(&queries[1]), "{accented:?}");
    }
    assert!(with_terms > 1500, "{with_terms} texts of 2000 had terms");
}

#[test]
fn analyzing_a_text_twice_gives_the_same_vector_bit_for_bit() {
    let average = AverageLength::new(7.5).unwrap();
    let mut draws = Draws(0x2545_f491_4f6c_dd1d);
    let mut with_terms = 0;
    for _ in 0..500 {
        let (text, _) = texts(&mut draws);
        let passage = bits(&Passage::new(&text).vector(average));
        assert_eq!(
            passage,
            bits(&Passage::new(&text).vector(average)),
            "{text:?}"
        );
        let query = bits(&query_vector(&text));
        assert_eq!(query, bits(&query_vector(&text)), "{text:?}");
        with_terms += usize::from(!passage.0.is_empty());
    }
    assert!(with_terms > 375, "{with_terms} texts of 500 had terms");
}
