//! The sparse vectors of `bm25-en-fr/1`: a passage's term weighs BM25's
//! term-frequency part, `tf·(k1+1) / (tf + k1·(1 − b + b·len/avg_len))` with
//! k1 1.2 and b 0.75, and a query's term weighs 1, each under the first four
//! bytes of the term's SHA-256, big-endian; indices are sorted and unique,
//! terms that share an ID add their weights, and a text without terms gives
//! an empty vector.
#![cfg(test)]

use maestro_knowledge::lexical::{AverageLength, Passage, SparseVector, query_vector};

/// The first four bytes of SHA-256 of `job`: `5e8c9902…`.
const JOB: u32 = 0x5e8c_9902;
/// The first four bytes of SHA-256 of `lot`: `b7b30d6e…`.
const LOT: u32 = 0xb7b3_0d6e;
/// The first four bytes of SHA-256 of both `k153629` and `k164064`.
const SHARED: u32 = 0xcaaf_373a;

/// `vector`'s indices and weights, paired.
fn pairs(vector: &SparseVector) -> Vec<(u32, f32)> {
    assert_eq!(vector.indices().len(), vector.values().len(), "{vector:?}");
    vector
        .indices()
        .iter()
        .copied()
        .zip(vector.values().iter().copied())
        .collect()
}

/// Whether `found` holds `expected`'s indices, with weights within a
/// millionth.
fn close(found: &[(u32, f32)], expected: &[(u32, f64)]) -> bool {
    found.len() == expected.len()
        && found.iter().zip(expected).all(|(found, expected)| {
            found.0 == expected.0 && (f64::from(found.1) - expected.1).abs() < 1e-6
        })
}

#[test]
fn a_passage_term_weighs_bm25_term_frequency_part() {
    let passage = Passage::new("job job lot");
    assert_eq!(passage.term_count(), 3);
    let vector = passage.vector(AverageLength::new(2.0).unwrap());
    // len/avg_len = 3/2, so the norm is 1.2 × (0.25 + 0.75 × 1.5) = 1.65:
    // `job` (tf 2) weighs 2 × 2.2 / 3.65, `lot` (tf 1) 2.2 / 2.65.
    let expected = [(JOB, 4.4 / 3.65), (LOT, 2.2 / 2.65)];
    assert!(close(&pairs(&vector), &expected), "{vector:?}");
}

#[test]
fn a_passage_of_average_length_weighs_a_single_term_one() {
    let vector = Passage::new("job lot").vector(AverageLength::new(2.0).unwrap());
    assert!(
        close(&pairs(&vector), &[(JOB, 1.0), (LOT, 1.0)]),
        "{vector:?}"
    );
}

#[test]
fn a_query_term_weighs_one_however_often_it_appears() {
    let vector = query_vector("jobs job lot");
    assert_eq!(pairs(&vector), [(JOB, 1.0), (LOT, 1.0)]);
}

#[test]
fn indices_are_sorted_and_unique() {
    let text = "The scheduler marks the run as failed once it reaches max_retries, \
                and the run is failed again.";
    for vector in [
        query_vector(text),
        Passage::new(text).vector(AverageLength::new(9.0).unwrap()),
    ] {
        let indices = vector.indices();
        assert!(indices.len() > 5, "{vector:?}");
        assert!(
            indices.windows(2).all(|pair| pair[0] < pair[1]),
            "{vector:?}"
        );
    }
}

#[test]
fn terms_sharing_an_id_add_their_weights() {
    assert_eq!(pairs(&query_vector("k153629 k164064")), [(SHARED, 2.0)]);
    let passage = Passage::new("k153629 k164064");
    assert_eq!(passage.term_count(), 2);
    let vector = passage.vector(AverageLength::new(2.0).unwrap());
    assert!(close(&pairs(&vector), &[(SHARED, 2.0)]), "{vector:?}");
}

#[test]
fn texts_without_terms_give_empty_vectors() {
    let average = AverageLength::new(8.0).unwrap();
    for text in ["", " \n\t ", "the of a", "le la les de", "à l’ été"] {
        let passage = Passage::new(text);
        assert_eq!(passage.term_count(), 0, "{text:?}");
        assert_eq!(pairs(&passage.vector(average)), [], "{text:?}");
        assert_eq!(pairs(&query_vector(text)), [], "{text:?}");
    }
}

#[test]
fn an_average_length_is_a_positive_finite_number() {
    for refused in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(AverageLength::new(refused), None, "{refused}");
    }
    assert!(AverageLength::new(0.5).is_some());
}
