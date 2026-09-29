//! Extractor cards: v2 only, with a pinned chat template, and no other role's calls.
//!
//! FR-S2-016: a `maestro-model-card/2` identity alone carries the role, with
//! the answerer's generating profile and a pinned chat template, and no
//! embed, rerank or answer call takes its card.

use super::{
    super::{
        CardError, Error, FakeModels, ModelCard, ModelPort, Role, Room,
        card_v2::{
            Capability, CardIdentity, Dimensions, EmbeddingFormat, Sampling, Template, TextFormat,
        },
    },
    fixture::{chat_request, fields},
    v2::{answerer_identity, digest, scratch_store},
    v2_validation::refuses,
};
use serde_json::Value;
use std::{fs, num::NonZeroUsize};

/// Why a v1 card refuses the extractor role.
const V2_ONLY: &str = "an extractor's card is maestro-model-card/2 only";

/// An extractor of the answerer fixture's model: the same generating profile,
/// with the chat template its server reports pinned.
fn extractor_identity() -> CardIdentity {
    let mut identity = answerer_identity();
    identity.role = Role::Extractor;
    identity.formats.template = Template::Digest(digest('d'));
    identity
}

#[test]
fn an_extractor_card_records_its_role_as_extractor_and_reads_back_unchanged() {
    let (path, store) = scratch_store();
    let card = ModelCard::record_v2(&store, &extractor_identity()).unwrap();
    assert_eq!(card.fields().role, Role::Extractor);
    assert_eq!(card.fields().template_digest, Some(digest('d')));
    assert_eq!(ModelCard::load(&store, card.digest()).unwrap(), card);
    let json: Value = serde_json::from_slice(&store.get(card.digest()).unwrap()).unwrap();
    assert_eq!(json["identity"]["role"], "extractor");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn an_extractor_needs_sampling_an_output_limit_a_chat_template_and_no_embedding() {
    let (path, store) = scratch_store();
    let refusal = "disagree with extractor role";
    let mut untemplated = extractor_identity();
    untemplated.formats.template = Template::Absent;
    refuses(&untemplated, &store, refusal);
    let mut unsampled = extractor_identity();
    unsampled.invocation.sampling = Sampling::NotApplicable;
    refuses(&unsampled, &store, refusal);
    let mut unbounded = extractor_identity();
    unbounded.invocation.limits.output_tokens = None;
    refuses(&unbounded, &store, refusal);
    let mut dimensioned = extractor_identity();
    dimensioned.invocation.dimensions = Dimensions::Measured(NonZeroUsize::new(8).unwrap());
    refuses(&dimensioned, &store, refusal);
    let mut pooled = extractor_identity();
    pooled.formats.embedding = EmbeddingFormat::Unsupported;
    refuses(&pooled, &store, refusal);
    let literal = Capability::Supported(TextFormat {
        prefix: String::new(),
        suffix: String::new(),
    });
    let mut document = extractor_identity();
    document.formats.document = literal.clone();
    refuses(&document, &store, refusal);
    let mut query = extractor_identity();
    query.formats.query = literal;
    refuses(&query, &store, refusal);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn an_answerer_card_of_the_same_model_is_another_card_than_its_extractor() {
    let (path, store) = scratch_store();
    let extractor = ModelCard::record_v2(&store, &extractor_identity()).unwrap();
    let mut answerer = extractor_identity();
    answerer.role = Role::Answerer;
    let answerer = ModelCard::record_v2(&store, &answerer).unwrap();
    assert_eq!(answerer.fields().role, Role::Answerer);
    assert_ne!(answerer.digest(), extractor.digest());
    assert_eq!(
        answerer.fields().file_digest,
        extractor.fields().file_digest
    );
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn a_v1_card_never_carries_the_extractor_role() {
    let (path, store) = scratch_store();
    let written = ModelCard::record(&store, &fields(Role::Extractor)).unwrap_err();
    assert!(
        matches!(&written, CardError::Invalid(reason) if reason.contains(V2_ONLY)),
        "{written}"
    );
    let bytes = concat!(
        r#"{"schema":"maestro-model-card/1","role":"extractor","router_entry":"extract","#,
        r#""file_digest":"ca56847038f3f329524caec5a86e14865f49918d68094c90"#,
        r#"dd021a5e67b927f6","template_digest":null,"server_build":"b6500-3f2c9a1b","#,
        r#""dimensions":null,"limits":{"context_tokens":8192,"#,
        r#""output_tokens":128},"suite_results":[]}"#,
    )
    .as_bytes();
    let stored = store.put(bytes).unwrap();
    let read = ModelCard::load(&store, &stored).unwrap_err();
    assert!(
        matches!(&read, CardError::Invalid(reason) if reason.contains(V2_ONLY)),
        "{read}"
    );
    fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn an_extractor_card_never_embeds_reranks_or_answers() {
    let (path, store) = scratch_store();
    let card = ModelCard::record_v2(&store, &extractor_identity()).unwrap();
    let texts = ["Whales sing.".to_owned()];
    let refusals = [
        (
            FakeModels
                .embed(&card, Room::Free, &texts)
                .await
                .unwrap_err(),
            Role::Embedder,
        ),
        (
            FakeModels
                .rerank(&card, Room::Free, "whale", &texts)
                .await
                .unwrap_err(),
            Role::Reranker,
        ),
        (
            FakeModels
                .chat(&card, Room::Free, &chat_request(&[]))
                .await
                .unwrap_err(),
            Role::Answerer,
        ),
    ];
    for (error, needed) in refusals {
        assert!(
            matches!(&error, Error::WrongRole { card: refused, role, needed: wanted }
                if refused == card.digest() && *role == Role::Extractor && *wanted == needed),
            "{error}"
        );
    }
    fs::remove_dir_all(path).unwrap();
}
