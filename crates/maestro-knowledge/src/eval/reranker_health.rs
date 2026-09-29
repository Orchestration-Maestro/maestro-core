//! Health qualification for rerankers, with public paired examples and a card-bound receipt.

use maestro_kernel::{
    artifact::Digest,
    gateway::{
        ModelCard, ModelPort, Role, Room,
        card_v2::{QualificationMethod, Template},
    },
    model::{Error, EvaluationDisposition, EvaluationMode, EvaluationRecord, NewModelEvaluation},
    scope::ScopeSet,
    store::Database,
};
use serde::Serialize;

/// Stable fixture set included in every reranker health-evaluation manifest.
const FIXTURE_MANIFEST: &[u8] =
    br#"{"schema":"maestro-reranker-health-fixtures/1","pairs":["photosynthesis","red-planet"]}"#;

/// Two independent public positive/negative pairs used before retrieval evaluation.
const PAIRS: [HealthPair; 2] = [
    HealthPair {
        id: "photosynthesis",
        query: "What gas do plants absorb during photosynthesis?",
        positive: "Plants absorb carbon dioxide during photosynthesis.",
        negative: "Plants use sunlight to power growth.",
    },
    HealthPair {
        id: "red-planet",
        query: "Which planet is known as the Red Planet?",
        positive: "Mars is commonly known as the Red Planet.",
        negative: "Saturn is known for its prominent rings.",
    },
];

/// One public positive/negative pair.
struct HealthPair {
    /// Stable, public fixture identifier.
    id: &'static str,
    /// The question scored against both passages.
    query: &'static str,
    /// Passage that answers the question.
    positive: &'static str,
    /// Passage that does not answer the question.
    negative: &'static str,
}

/// The result of the two-pair health gate, including its inspectable receipt.
#[derive(Debug, Clone)]
pub struct RerankerHealth {
    /// Digest of the exact card used for both model calls.
    card_digest: Digest,
    /// The combined disposition of the public fixture pair checks.
    disposition: EvaluationDisposition,
    /// Scores and provenance retained for audit.
    receipt: HealthReceipt,
}

impl RerankerHealth {
    /// Final disposition; only two valid pair margins produce `Eligible`.
    #[must_use]
    pub const fn disposition(&self) -> EvaluationDisposition {
        self.disposition
    }

    /// Digest of the exact card used for both model calls.
    #[must_use]
    pub fn card_digest(&self) -> &Digest {
        &self.card_digest
    }

    /// Public-fixture scores and card/runtime pin recorded by the gate.
    #[must_use]
    pub const fn receipt(&self) -> &HealthReceipt {
        &self.receipt
    }

    /// First failed health-gate condition, if any.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.receipt.failure.as_deref()
    }
}

/// The registry inputs for one real health evaluation.
#[derive(Debug)]
pub struct RerankerEvaluation<'a> {
    /// The collection that registered the candidate card.
    pub collection_id: &'a str,
    /// Stable identifier for this health run.
    pub run_id: &'a str,
    /// Generation whose retrieval is being evaluated, if one exists.
    pub generation_id: Option<i64>,
    /// Additional reproducibility metadata; an empty value uses the fixture manifest.
    pub manifest: &'a [u8],
}

/// Card-bound health evidence stored as the real evaluation report.
#[derive(Debug, Clone, Serialize)]
pub struct HealthReceipt {
    /// The receipt schema version.
    schema: &'static str,
    /// The complete model-card digest under test.
    card_digest: Digest,
    /// The card's model-weights digest, absent for a non-v2 card.
    weights_digest: Option<Digest>,
    /// The card's runtime-binary digest, absent for a non-v2 card.
    runtime_digest: Option<Digest>,
    /// The card's pinned runtime build, absent for a non-v2 card.
    runtime_build: Option<String>,
    /// The template asset digest when the card uses a pinned template.
    template_digest: Option<Digest>,
    /// The declared qualification method, absent for a non-v2 card.
    qualification_method: Option<&'static str>,
    /// Scores observed for the pairs reached before the first failure.
    pairs: Vec<PairReceipt>,
    /// The first condition that prevented eligibility.
    failure: Option<String>,
}

/// Scores observed for one public fixture pair.
#[derive(Debug, Clone, Serialize)]
struct PairReceipt {
    /// Stable fixture-pair identifier.
    id: &'static str,
    /// Finite answer score, or absent when nonfinite or unavailable.
    positive_score: Option<f64>,
    /// Finite distractor score, or absent when nonfinite or unavailable.
    negative_score: Option<f64>,
    /// Finite answer-minus-distractor spread, when available.
    spread: Option<f64>,
}

/// Scores the card on both built-in public pairs.
///
/// Each pair needs exactly two finite scores, positive before negative, with
/// positive-minus-negative spread of at least 0.1. The card identity pins the
/// weights, runtime binary, build and template; only native-runtime-qualified
/// v2 reranker cards may be tested. Head presence is established by score
/// behavior, not by parsing model-specific tensor metadata.
///
/// # Errors
///
/// This gate returns a `Blocked`, `Ineligible` or `Failed` disposition in its
/// receipt rather than returning an error. A runtime refusal is `Failed`.
pub async fn check_reranker_health<P: ModelPort>(port: &P, card: &ModelCard) -> RerankerHealth {
    let card_digest = card.digest().clone();
    let mut receipt = HealthReceipt {
        schema: "maestro-reranker-health/1",
        card_digest: card_digest.clone(),
        weights_digest: None,
        runtime_digest: None,
        runtime_build: None,
        template_digest: None,
        qualification_method: None,
        pairs: Vec::new(),
        failure: None,
    };
    let Some(identity) = card.identity() else {
        receipt.failure = Some("health qualification requires a v2 card".to_owned());
        return health(card_digest, EvaluationDisposition::Blocked, receipt);
    };
    receipt.weights_digest = Some(identity.weights.gguf_digest.clone());
    receipt.runtime_digest = Some(identity.invocation.runtime_binary_digest.clone());
    receipt.runtime_build = Some(identity.invocation.llama_cpp_build.clone());
    receipt.template_digest = match &identity.formats.template {
        Template::Digest(digest) => Some(digest.clone()),
        Template::Absent => None,
    };
    receipt.qualification_method = Some(match identity.provenance.qualification_method {
        QualificationMethod::NativeRuntime => "native_runtime",
        QualificationMethod::NativeTokenizer => "native_tokenizer",
        QualificationMethod::PublisherEvidence => "publisher_evidence",
    });
    if identity.role != Role::Reranker {
        receipt.failure = Some("health qualification requires a reranker card".to_owned());
        return health(card_digest, EvaluationDisposition::Blocked, receipt);
    }
    if identity.provenance.qualification_method != QualificationMethod::NativeRuntime {
        receipt.failure =
            Some("health qualification requires native runtime provenance".to_owned());
        return health(card_digest, EvaluationDisposition::Blocked, receipt);
    }

    for pair in PAIRS {
        let documents = [pair.positive.to_owned(), pair.negative.to_owned()];
        let scores = match port.rerank(card, Room::Free, pair.query, &documents).await {
            Ok(scores) => scores,
            Err(error) => {
                receipt.failure = Some(format!("{}: {error}", pair.id));
                return health(card_digest, EvaluationDisposition::Failed, receipt);
            }
        };
        let [positive, negative] = scores.as_slice() else {
            receipt.failure = Some(format!(
                "{}: expected two scores, received {}",
                pair.id,
                scores.len()
            ));
            return health(card_digest, EvaluationDisposition::Ineligible, receipt);
        };
        let positive_score = positive.is_finite().then_some(*positive);
        let negative_score = negative.is_finite().then_some(*negative);
        let spread = positive_score
            .zip(negative_score)
            .map(|(positive, negative)| positive - negative)
            .filter(|spread| spread.is_finite());
        let failure = if positive_score.is_none() || negative_score.is_none() {
            Some("pair scores must be finite")
        } else if positive_score
            .zip(negative_score)
            .is_some_and(|(positive, negative)| positive <= negative)
        {
            Some("positive score must exceed negative score")
        } else if spread.is_none_or(|spread| spread < 0.1) {
            Some("pair spread must be at least 0.1")
        } else {
            None
        };
        receipt.pairs.push(PairReceipt {
            id: pair.id,
            positive_score,
            negative_score,
            spread,
        });
        if let Some(failure) = failure {
            receipt.failure = Some(format!("{}: {failure}", pair.id));
            return health(card_digest, EvaluationDisposition::Ineligible, receipt);
        }
    }
    health(card_digest, EvaluationDisposition::Eligible, receipt)
}

/// Scores the candidate and records either its health result or its failure as real evidence.
///
/// Only a passing two-pair gate records `Eligible`; failing health is recorded with its
/// `Ineligible`, `Blocked` or `Failed` disposition and card-bound receipt.
///
/// # Errors
///
/// Returns a registry error if the card is unregistered or the database refuses the evidence.
pub async fn evaluate_and_record_reranker_health<P: ModelPort>(
    database: &Database,
    scopes: &ScopeSet,
    card: &ModelCard,
    port: &P,
    evaluation: &RerankerEvaluation<'_>,
) -> Result<(RerankerHealth, EvaluationRecord), Error> {
    let health = check_reranker_health(port, card).await;
    let record = record_reranker_health(database, scopes, card, &health, evaluation)?;
    Ok((health, record))
}

/// Records the gate outcome against the exact registered card as a real evaluation.
///
/// # Errors
///
/// Returns a registry error if the receipt belongs to another card, the card is unregistered,
/// or the database refuses the evidence.
pub fn record_reranker_health(
    database: &Database,
    scopes: &ScopeSet,
    card: &ModelCard,
    health: &RerankerHealth,
    evaluation: &RerankerEvaluation<'_>,
) -> Result<EvaluationRecord, Error> {
    if card.digest() != &health.card_digest {
        return Err(Error::Invalid(
            "health receipt belongs to another model card".to_owned(),
        ));
    }
    let card_record = database
        .model_cards(scopes, evaluation.collection_id, Role::Reranker)?
        .into_iter()
        .find(|record| record.digest == health.card_digest)
        .ok_or_else(|| Error::Invalid("health card is not registered as a reranker".to_owned()))?;
    let report = serde_json::to_vec(&health.receipt)
        .map_err(|error| Error::Invalid(format!("health receipt is not serializable: {error}")))?;
    database.record_model_evaluation(
        scopes,
        &NewModelEvaluation {
            run_id: evaluation.run_id,
            collection_id: evaluation.collection_id,
            card_id: card_record.id,
            role: Role::Reranker,
            mode: EvaluationMode::Real,
            generation_id: evaluation.generation_id,
            disposition: health.disposition,
            manifest: if evaluation.manifest.is_empty() {
                FIXTURE_MANIFEST
            } else {
                evaluation.manifest
            },
            report: &report,
        },
    )
}

/// Couples the card pin and disposition to the receipt.
fn health(
    card_digest: Digest,
    disposition: EvaluationDisposition,
    receipt: HealthReceipt,
) -> RerankerHealth {
    RerankerHealth {
        card_digest,
        disposition,
        receipt,
    }
}
