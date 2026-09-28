//! Draft candidates stay unreviewed and bounded by approved source windows.
use super::support::{REVISION, SOURCE, SOURCE_REF, labels, text};
use crate::eval::graph::draft::{DraftWindow, check_candidate, checked_chat};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Approved synthetic source window.
pub(super) fn window() -> DraftWindow {
    DraftWindow {
        id: "q-1".into(),
        source_ref: SOURCE_REF.into(),
        revision_id: REVISION.into(),
        original: Digest::of(SOURCE.as_bytes()),
        span: [0, SOURCE.len()],
        source: SOURCE.into(),
    }
}

/// One draft encoded as the gateway's strict envelope.
fn candidate() -> Value {
    let mut label = labels().remove(0);
    label["review"] = Value::Null;
    let suite = json!({"schema":"maestro-suite/1", "id":"q-1", "language":"en",
        "question":"What is the retries default?", "answerable":true,
        "expected":[{"source_ref": SOURCE_REF, "heading_path":["Lantern"]}]});
    json!({"suite":suite.to_string(), "labels":text(&[label])})
}

#[test]
fn draft_revalidates_anchors_and_retains_unreviewed_labels() {
    let draft = check_candidate(&candidate().to_string(), &window(), &BTreeSet::new()).unwrap();
    assert_eq!(draft.id, "q-1");
    assert_eq!(draft.family, "f-1");
    assert!(draft.labels.contains("\"review\":null"));
}

#[test]
fn draft_refuses_invented_anchors_reviews_and_duplicate_families() {
    let mut families = BTreeSet::new();
    families.insert("f-1".to_owned());
    assert!(check_candidate(&candidate().to_string(), &window(), &families).is_err());
    for (field, value) in [
        (
            "review",
            json!({"reviewer_card":"invented", "disposition":"accepted"}),
        ),
        ("proofs", json!([])),
    ] {
        let mut output = candidate();
        let mut label: Value = serde_json::from_str(output["labels"].as_str().unwrap()).unwrap();
        label[field] = value;
        output["labels"] = json!(label.to_string());
        assert!(check_candidate(&output.to_string(), &window(), &BTreeSet::new()).is_err());
    }
    let mut outside = window();
    outside.span = [0, 10];
    assert!(check_candidate(&candidate().to_string(), &outside, &BTreeSet::new()).is_err());
}

#[test]
fn draft_rejects_changed_source_and_malformed_private_text_safely() {
    let mut changed = window();
    changed.source.push_str("changed");
    assert!(check_candidate(&candidate().to_string(), &changed, &BTreeSet::new()).is_err());
    let failure =
        check_candidate("SECRET question quote", &window(), &BTreeSet::new()).unwrap_err();
    assert!(!format!("{failure:?} {failure}").contains("SECRET"));
}

use crate::eval::graph::draft::{DraftBudget, DraftError, DraftRequest, draft_window};
use maestro_kernel::{
    artifact::Store,
    gateway::{
        CardFields, ChatRequest, Error, FakeModels, Limits, ModelCard, ModelPort, Role, Room,
        RouterEntry,
    },
};
use std::{
    env, fs, future,
    num::NonZeroU32,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use std::{future::Future, process};

/// Scripted inference; every call asserts free-room policy.
pub(super) struct DraftModel {
    reply: Mutex<Option<Result<String, Error>>>,
    pub(super) chats: AtomicUsize,
    tokens: usize,
    pub(super) calls: AtomicUsize,
    framing: usize,
    hangs: bool,
}
impl DraftModel {
    /// One successful synthetic candidate.
    pub(super) fn valid() -> Self {
        Self {
            reply: Mutex::new(Some(Ok(candidate().to_string()))),
            chats: AtomicUsize::new(0),
            tokens: 1,
            calls: AtomicUsize::new(0),
            framing: 1,
            hangs: false,
        }
    }
}
impl ModelPort for DraftModel {
    fn prepare(&self, _card: &ModelCard, room: Room) -> impl Future<Output = Result<(), Error>> {
        assert_eq!(room, Room::Free);
        self.calls.fetch_add(1, Ordering::SeqCst);
        future::ready(Ok(()))
    }
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        FakeModels.embed(card, room, inputs).await
    }
    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        FakeModels.rerank(card, room, query, documents).await
    }
    fn tokenize(
        &self,
        _card: &ModelCard,
        room: Room,
        text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> {
        assert_eq!(room, Room::Free);
        self.calls.fetch_add(1, Ordering::SeqCst);
        future::ready(Ok(vec![
            0;
            self.tokens
                + if text == "rendered" {
                    self.framing
                } else {
                    0
                }
        ]))
    }
    fn render_chat(
        &self,
        _card: &ModelCard,
        room: Room,
        _request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> {
        assert_eq!(room, Room::Free);
        self.calls.fetch_add(1, Ordering::SeqCst);
        future::ready(Ok("rendered".into()))
    }
    async fn chat(
        &self,
        _card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> Result<String, Error> {
        assert_eq!(room, Room::Free);
        assert_eq!(request.max_output_tokens, 1024);
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.chats.fetch_add(1, Ordering::SeqCst);
        if self.hangs {
            return future::pending().await;
        }
        self.reply.lock().unwrap().take().unwrap()
    }
}

/// Immutable synthetic card; its temporary artifact is removed immediately.
pub(super) fn card() -> ModelCard {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = env::temp_dir().join(format!(
        "maestro-draft-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let fields = CardFields {
        role: Role::Answerer,
        router_entry: RouterEntry::parse("synthetic-drafter").unwrap(),
        file_digest: Digest::of(b"model"),
        template_digest: None,
        server_build: "synthetic".into(),
        dimensions: None,
        limits: Limits {
            context_tokens: NonZeroU32::new(4096).unwrap(),
            output_tokens: NonZeroU32::new(1024),
        },
        suite_results: vec![],
    };
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(root).unwrap();
    card
}

/// Explicit bounds used by tests, not a production default.
pub(super) fn budget() -> DraftBudget {
    DraftBudget {
        input_bytes: 4096,
        input_tokens: 2048,
        output_tokens: 1024,
        output_bytes: 4096,
        deadline_ms: 1000,
    }
}

#[tokio::test]
async fn draft_calls_the_pinned_model_with_free_room_and_private_payload() {
    let card = card();
    let window = window();
    let prompt_digest = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt_digest,
        window: &window,
        budget: budget(),
    };
    let model = DraftModel::valid();
    let draft = draft_window(&model, &request, &BTreeSet::new())
        .await
        .unwrap();
    assert_eq!(draft.id, "q-1");
    assert_eq!(model.chats.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn draft_refuses_changed_pins_and_exceeded_limits_before_chat() {
    let card = card();
    let window = window();
    let prompt_digest = Digest::of(b"draft");
    let wrong = Digest::of(b"changed");
    let mut request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt_digest,
        window: &window,
        budget: budget(),
    };
    let model = DraftModel::valid();
    request.card_digest = &wrong;
    assert_eq!(
        draft_window(&model, &request, &BTreeSet::new())
            .await
            .unwrap_err(),
        DraftError::Card
    );
    request.card_digest = card.digest();
    request.prompt_digest = &wrong;
    assert_eq!(
        draft_window(&model, &request, &BTreeSet::new())
            .await
            .unwrap_err(),
        DraftError::Prompt
    );
    request.prompt_digest = &prompt_digest;
    for limited in [
        DraftBudget {
            input_bytes: 1,
            ..budget()
        },
        DraftBudget {
            input_tokens: 1,
            ..budget()
        },
        DraftBudget {
            output_tokens: 1025,
            ..budget()
        },
        DraftBudget {
            deadline_ms: 0,
            ..budget()
        },
    ] {
        request.budget = limited;
        assert_eq!(
            draft_window(&model, &request, &BTreeSet::new())
                .await
                .unwrap_err(),
            DraftError::Budget
        );
    }
    assert_eq!(model.chats.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn draft_sanitizes_room_refusals_timeouts_and_output_overflow() {
    let card = card();
    let window = window();
    let prompt_digest = Digest::of(b"draft");
    let mut request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt_digest,
        window: &window,
        budget: budget(),
    };
    let mut model = DraftModel::valid();
    *model.reply.lock().unwrap() = Some(Err(Error::Unavailable {
        reason: "SECRET nested router error".into(),
    }));
    let error = draft_window(&model, &request, &BTreeSet::new())
        .await
        .unwrap_err();
    assert_eq!(error, DraftError::Gateway);
    assert_eq!(error.to_string(), "draft_gateway");
    model.hangs = true;
    assert_eq!(
        draft_window(&model, &request, &BTreeSet::new())
            .await
            .unwrap_err(),
        DraftError::Timeout
    );
    request.budget.output_bytes = 1;
    assert_eq!(
        draft_window(&DraftModel::valid(), &request, &BTreeSet::new())
            .await
            .unwrap_err(),
        DraftError::Budget
    );
    request.budget = budget();
    let mut model = DraftModel::valid();
    model.tokens = 1025;
    request.budget.input_tokens = 2050;
    assert_eq!(
        draft_window(&model, &request, &BTreeSet::new())
            .await
            .unwrap_err(),
        DraftError::Budget
    );
    assert_eq!(model.chats.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn draft_charges_template_framing_before_chat() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: DraftBudget {
            input_tokens: 2,
            ..budget()
        },
    };
    let mut model = DraftModel::valid();
    model.framing = 2;
    assert_eq!(
        draft_window(&model, &request, &BTreeSet::new())
            .await
            .unwrap_err(),
        DraftError::Budget
    );
    assert_eq!(model.chats.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn draft_byte_and_token_limits_accept_exact_fit_and_refuse_one_over() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let mut request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    let chat = checked_chat(&request).unwrap();
    let input_bytes: usize = chat
        .messages
        .iter()
        .map(|message| message.content.len())
        .sum();
    let output_bytes = candidate().to_string().len();
    for (boundary, excess) in [
        ("input bytes", 0),
        ("input bytes", 1),
        ("output bytes", 0),
        ("output bytes", 1),
        ("output tokens", 0),
        ("output tokens", 1),
    ] {
        request.budget = budget();
        let mut model = DraftModel::valid();
        match boundary {
            "input bytes" => request.budget.input_bytes = input_bytes - excess,
            "output bytes" => request.budget.output_bytes = output_bytes - excess,
            "output tokens" => model.tokens = 1024 + excess,
            _ => panic!("unknown boundary"),
        }
        let result = draft_window(&model, &request, &BTreeSet::new()).await;
        if excess == 0 {
            assert!(result.is_ok(), "{boundary}: {result:?}");
        } else {
            assert_eq!(result.unwrap_err(), DraftError::Budget, "{boundary}");
        }
    }
}

#[tokio::test]
async fn draft_refuses_adapters_without_template_rendering() {
    let card = card();
    let window = window();
    let prompt = Digest::of(b"draft");
    let request = DraftRequest {
        card: &card,
        card_digest: card.digest(),
        prompt: "draft",
        prompt_digest: &prompt,
        window: &window,
        budget: budget(),
    };
    assert_eq!(
        draft_window(&FakeModels, &request, &BTreeSet::new())
            .await
            .unwrap_err(),
        DraftError::Unsupported
    );
}
