//! Research R7's public sample on `bm25-en-fr/1`: 22 passages, 11 in English
//! and 11 in French, and the 23 checks that no configuration of Qdrant's own
//! BM25 passes, with no language given for any passage or query.
//!
//! A passage scores as Qdrant 1.19 scores a sparse vector declared with
//! `modifier: idf`: the sum, over the query's terms, of the term's IDF over
//! the 22 passages times the passage's weight for it, the passages weighed
//! with the sample's average length. A recall check passes when every
//! expected passage scores above zero; an identifier or topic check passes
//! when every expected passage scores strictly above every other passage:
//! scores, not ranks, since Qdrant orders tied scores arbitrarily.
#![cfg(test)]

use maestro_knowledge::lexical::{AverageLength, Passage, SparseVector, query_vector};
use std::collections::BTreeMap;

/// The sample of `specs/001-knowledge-kernel/research.md`, as written there.
const PASSAGES: [(&str, &str); 22] = [
    (
        "en01",
        "The scheduler marks the run as failed once it reaches max_retries.",
    ),
    ("en02", "Error ERR-4012: the request header is missing."),
    (
        "en03",
        "Error ERR-4013 is raised when the worker cannot reach the queue.",
    ),
    (
        "en04",
        "The max timeout is thirty seconds, and retries are logged by the worker.",
    ),
    (
        "en05",
        "Every request must carry a job-id header so the gateway can trace it.",
    ),
    (
        "en06",
        "Each job receives an id when it is created and keeps it until deletion.",
    ),
    (
        "en07",
        "AgentPort is the interface that connects a worker to the scheduler.",
    ),
    ("en08", "The agent listens on a port chosen at startup."),
    (
        "en09",
        "Planned tasks keep their results in the artifact store.",
    ),
    ("en10", "A task writes one result when it completes."),
    (
        "en11",
        "Retry policy: failed jobs are retried with exponential backoff.",
    ),
    (
        "fr01",
        "Le planificateur abandonne après max_retries tentatives et marque \
         l'exécution comme échouée.",
    ),
    (
        "fr02",
        "L'erreur ERR-4012 signale qu'un en-tête manque dans la requête.",
    ),
    (
        "fr03",
        "Chaque requête doit porter l'en-tête (job-id) pour être tracée.",
    ),
    (
        "fr04",
        "Grâce à AgentPort, chaque exécutant rejoint le planificateur.",
    ),
    (
        "fr05",
        "Le résultat de la tâche est enregistré dans le magasin d'artefacts.",
    ),
    (
        "fr06",
        "Les résultats des tâches planifiées sont conservés trente jours.",
    ),
    ("fr07", "Une exécution planifiée démarre à l'heure prévue."),
    (
        "fr08",
        "Il faut planifier le lot avant de lancer l'exécution.",
    ),
    (
        "fr09",
        "Journal sans accents : resultat de la tache planifiee, statut termine.",
    ),
    (
        "fr10",
        "Politique de relance : les travaux échoués sont relancés avec un délai exponentiel.",
    ),
    ("fr11", "Le lot a été planifié hier soir par l'opératrice."),
];

/// A check: its query and the passages it expects.
type Check = (&'static str, &'static [&'static str]);

/// A passage's ID and its score for a query.
type Score = (&'static str, f32);

/// The sample's passages as Qdrant would hold them.
struct Sample {
    /// Each passage's ID and sparse vector, in the order of [`PASSAGES`].
    vectors: Vec<(&'static str, SparseVector)>,
    /// How many passages hold each token ID.
    holding: BTreeMap<u32, u16>,
}

impl Sample {
    fn new() -> Self {
        let passages: Vec<(&str, Passage)> = PASSAGES
            .iter()
            .map(|&(id, text)| (id, Passage::new(text)))
            .collect();
        let terms: usize = passages
            .iter()
            .map(|(_, passage)| passage.term_count())
            .sum();
        let terms = f64::from(u32::try_from(terms).unwrap());
        let count = f64::from(u32::try_from(passages.len()).unwrap());
        let average = AverageLength::new(terms / count).unwrap();
        let vectors: Vec<(&str, SparseVector)> = passages
            .iter()
            .map(|(id, passage)| (*id, passage.vector(average)))
            .collect();
        let mut holding = BTreeMap::new();
        for index in vectors.iter().flat_map(|(_, vector)| vector.indices()) {
            *holding.entry(*index).or_insert(0) += 1;
        }
        Self { vectors, holding }
    }

    /// Each passage's score for `query`, with its ID.
    fn scores(&self, query: &str) -> Vec<Score> {
        let query = query_vector(query);
        let indexed = f32::from(u16::try_from(self.vectors.len()).unwrap());
        let weighed: Vec<(u32, f32)> = query
            .indices()
            .iter()
            .zip(query.values())
            .map(|(index, weight)| {
                let holding = f32::from(self.holding.get(index).copied().unwrap_or(0));
                (*index, weight * idf(indexed, holding))
            })
            .collect();
        self.vectors
            .iter()
            .map(|(id, vector)| (*id, dot(&weighed, vector)))
            .collect()
    }
}

/// The IDF Qdrant 1.19 multiplies a query's weights by under
/// `modifier: idf`: `VectorQueryContext::fancy_idf` in
/// `lib/segment/src/data_types/query_context.rs` at tag v1.19.1,
/// `ln((n - df + 0.5) / (df + 0.5) + 1)` in `f32`, where `n` counts the
/// indexed vectors and `df` those holding the token.
fn idf(indexed: f32, holding: f32) -> f32 {
    ((indexed - holding + 0.5) / (holding + 0.5) + 1.0).ln()
}

/// The dot product of the IDF-weighed query and a passage's vector.
fn dot(query: &[(u32, f32)], passage: &SparseVector) -> f32 {
    passage
        .indices()
        .iter()
        .zip(passage.values())
        .filter_map(|(index, weight)| {
            query
                .iter()
                .find(|(term, _)| term == index)
                .map(|(_, query_weight)| query_weight * weight)
        })
        .sum()
}

/// The recall checks among `checks` that fail: some expected passage scores
/// zero.
fn failed_recall(checks: &[Check]) -> Vec<String> {
    let sample = Sample::new();
    checks
        .iter()
        .filter_map(|&(query, expected)| {
            let scores = sample.scores(query);
            let missed: Vec<&str> = scores
                .iter()
                .filter(|(id, score)| expected.contains(id) && *score <= 0.0)
                .map(|(id, _)| *id)
                .collect();
            (!missed.is_empty()).then(|| format!("{query:?} misses {missed:?}: {scores:?}"))
        })
        .collect()
}

/// The ranking checks among `checks` that fail: some expected passage scores
/// no higher than some other passage.
fn failed_ranking(checks: &[Check]) -> Vec<String> {
    let sample = Sample::new();
    checks
        .iter()
        .filter_map(|&(query, expected)| {
            let scores = sample.scores(query);
            let (found, others): (Vec<Score>, Vec<Score>) = scores
                .into_iter()
                .partition(|(id, _)| expected.contains(id));
            let lowest = found
                .iter()
                .map(|(_, score)| *score)
                .fold(f32::INFINITY, f32::min);
            let rivals: Vec<Score> = others
                .into_iter()
                .filter(|(_, score)| *score >= lowest)
                .collect();
            (!rivals.is_empty()).then(|| format!("{query:?} ranks {rivals:?} as high as {found:?}"))
        })
        .collect()
}

#[test]
fn accented_and_folded_queries_find_the_same_passages() {
    let failed = failed_recall(&[
        ("resultat", &["fr05", "fr06", "fr09"]),
        ("résultat", &["fr05", "fr06", "fr09"]),
        ("tache", &["fr05", "fr06", "fr09"]),
        ("tâche", &["fr05", "fr06", "fr09"]),
        ("planifiee", &["fr06", "fr07", "fr08", "fr09", "fr11"]),
        ("execution", &["fr01", "fr07", "fr08"]),
    ]);
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn plural_queries_find_their_singular_passages() {
    let failed = failed_recall(&[
        ("task", &["en09", "en10"]),
        ("results", &["en09", "en10"]),
        ("retry", &["en04", "en11"]),
        ("résultats", &["fr05", "fr06", "fr09"]),
        ("tâches", &["fr05", "fr06", "fr09"]),
    ]);
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn inflected_french_verbs_find_their_other_forms() {
    let failed = failed_recall(&[
        ("planifier", &["fr06", "fr07", "fr08", "fr09", "fr11"]),
        ("planifiés", &["fr06", "fr07", "fr08", "fr09", "fr11"]),
        ("relancer", &["fr10"]),
        ("échouées", &["fr01", "fr10"]),
    ]);
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn identifier_queries_rank_their_passages_above_the_decoys() {
    let failed = failed_ranking(&[
        ("ERR-4012", &["en02", "fr02"]),
        ("max_retries", &["en01", "fr01"]),
        ("job-id", &["en05", "fr03"]),
        ("AgentPort", &["en07", "fr04"]),
        ("agentport", &["en07", "fr04"]),
    ]);
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn topic_queries_rank_their_passage_above_the_others() {
    let failed = failed_ranking(&[
        ("how are failed jobs retried", &["en11"]),
        ("comment les travaux échoués sont-ils relancés", &["fr10"]),
        ("travaux echoues relances", &["fr10"]),
    ]);
    assert!(failed.is_empty(), "{failed:#?}");
}
