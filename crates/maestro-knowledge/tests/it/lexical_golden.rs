//! The golden of `bm25-en-fr/1`: the terms and vectors of sample passages and
//! queries, pinned bit for bit. A generation records the profile its vectors
//! were computed with, and a query is analyzed with that profile, so a rule
//! change that breaks this test is a new profile, `bm25-en-fr/2`, with a
//! golden of its own: never a new golden under this name.
#![cfg(test)]

use maestro_knowledge::lexical::{
    AverageLength, PROFILE, Passage, SparseVector, query_vector, terms,
};

/// `vector`'s indices and the bits of its weights, paired.
fn bits(vector: &SparseVector) -> Vec<(u32, u32)> {
    let weights = vector.values().iter().map(|weight| weight.to_bits());
    vector.indices().iter().copied().zip(weights).collect()
}

/// A passage's terms and its vector with an average length of 8 terms, as
/// pinned.
fn passage(text: &str) -> (Vec<String>, Vec<(u32, u32)>) {
    let average = AverageLength::new(8.0).unwrap();
    (terms(text), bits(&Passage::new(text).vector(average)))
}

#[test]
fn the_golden_belongs_to_profile_bm25_en_fr_1() {
    assert_eq!(PROFILE, "bm25-en-fr/1");
}

#[test]
fn golden_english_passages_keep_their_terms_and_vectors() {
    let (found, vector) =
        passage("The scheduler marks the run as failed once it reaches max_retries.");
    let expected = [
        "schedul",
        "mark",
        "run",
        "fail",
        "onc",
        "reach",
        "max_retries",
        "max",
        "retri",
    ];
    assert_eq!(found, expected);
    // Nine terms, each once: 2.2 / (1 + 1.2 × (0.25 + 0.75 × 9 / 8)).
    let weight = 0x3f73_8bc3;
    let expected = [
        (0x1c4c_5788, weight),
        (0x28e6_f59d, weight),
        (0x5128_0dab, weight),
        (0x6201_eb4d, weight),
        (0x9baf_3a40, weight),
        (0xacba_2551, weight),
        (0xb1fe_d513, weight),
        (0xd37a_e73c, weight),
        (0xed45_61f6, weight),
    ];
    assert_eq!(vector, expected);
    let (found, vector) =
        passage("Retry policy: failed jobs are retried with exponential backoff.");
    let expected = [
        "retri",
        "polici",
        "fail",
        "job",
        "retri",
        "exponential",
        "backoff",
    ];
    assert_eq!(found, expected);
    // Seven terms, `retri` twice.
    let expected = [
        (0x42b6_c478, 0x3f86_e5f1),
        (0x5128_0dab, 0x3f86_e5f1),
        (0x5e8c_9902, 0x3f86_e5f1),
        (0xb1fe_d513, 0x3fb6_69b7),
        (0xb70e_f15f, 0x3f86_e5f1),
        (0xd075_063d, 0x3f86_e5f1),
    ];
    assert_eq!(vector, expected);
}

#[test]
fn golden_french_passages_keep_their_terms_and_vectors() {
    let (found, vector) =
        passage("Chaque requête doit porter l'en-tête (job-id) pour être tracée.");
    let expected = [
        "chaqu", "requet", "port", "en-tete", "tet", "job-id", "job", "id", "trac",
    ];
    assert_eq!(found, expected);
    let weight = 0x3f73_8bc3;
    let expected = [
        (0x0521_425d, weight),
        (0x0c67_b2c1, weight),
        (0x4950_12c7, weight),
        (0x5e8c_9902, weight),
        (0x9f68_e2ed, weight),
        (0xa561_4527, weight),
        (0xc912_eef2, weight),
        (0xed37_320b, weight),
        (0xf8d3_97a3, weight),
    ];
    assert_eq!(vector, expected);
    let (found, vector) = passage("Une exécution planifiée démarre à l'heure prévue.");
    assert_eq!(found, ["execution", "planifi", "demar", "heur", "prevu"]);
    // Five terms: 2.2 / (1 + 1.2 × (0.25 + 0.75 × 5 / 8)).
    let weight = 0x3f97_31d3;
    let expected = [
        (0x08cf_1d6c, weight),
        (0x16f2_1639, weight),
        (0x242d_f896, weight),
        (0xa402_c433, weight),
        (0xa872_02e3, weight),
    ];
    assert_eq!(vector, expected);
}

#[test]
fn golden_queries_keep_their_terms_and_vectors() {
    let one = 1.0_f32.to_bits();
    let queries: [(&str, &[&str], &[u32]); 3] = [
        (
            "AgentPort",
            &["agentport", "agent", "port"],
            &[0x3c3d_8829, 0xd4f0_bc5a, 0xf8d3_97a3],
        ),
        ("planifiés", &["planifi"], &[0x08cf_1d6c]),
        (
            "comment les travaux échoués sont-ils relancés",
            &["comment", "traval", "echou", "sont-ils", "relanc"],
            &[
                0x07aa_af5d,
                0x1f60_5015,
                0xc44b_b2fd,
                0xcbfc_2445,
                0xe9d5_7a1d,
            ],
        ),
    ];
    for (query, expected_terms, expected_indices) in queries {
        assert_eq!(terms(query), expected_terms, "{query:?}");
        let expected: Vec<(u32, u32)> =
            expected_indices.iter().map(|index| (*index, one)).collect();
        assert_eq!(bits(&query_vector(query)), expected, "{query:?}");
    }
}
