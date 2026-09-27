use crate::search::{Candidate, DEFAULT_DEPTH, Fused, Ranked, Reranked, Reranker, rerank};
use maestro_kernel::{
    artifact::{Digest, Store},
    evidence::RouteStatus,
    gateway::{
        CardFields, ChatRequest, Error, Limits, ModelCard, ModelPort, Role, Room, RouterEntry,
        SuiteResult,
    },
};
use std::{
    collections::BTreeMap,
    env, fs,
    future::{self, Future},
    num::{NonZeroU32, NonZeroUsize},
    process,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::time::sleep;

pub(super) struct FakePort {
    reply: Reply,
    delay: Duration,
    pub(super) calls: Mutex<Vec<RerankCall>>,
    tokenized: Mutex<Vec<String>>,
    token_overrides: Vec<(String, usize)>,
}

#[derive(Clone)]
enum Reply {
    Scores(Vec<f64>),
    NoRoom,
}

pub(super) struct RerankCall {
    pub(super) room: Room,
    pub(super) query: String,
    pub(super) documents: Vec<String>,
}

impl FakePort {
    pub(super) fn scores(scores: Vec<f64>) -> Self {
        Self::new(Reply::Scores(scores), Duration::ZERO)
    }

    pub(super) fn delayed_scores(scores: Vec<f64>, delay: Duration) -> Self {
        Self::new(Reply::Scores(scores), delay)
    }

    fn new(reply: Reply, delay: Duration) -> Self {
        Self {
            reply,
            delay,
            calls: Mutex::new(Vec::new()),
            tokenized: Mutex::new(Vec::new()),
            token_overrides: Vec::new(),
        }
    }

    pub(super) fn with_token_overrides(mut self, overrides: &[(&str, usize)]) -> Self {
        self.token_overrides = overrides
            .iter()
            .map(|(text, count)| ((*text).to_owned(), *count))
            .collect();
        self
    }
}

impl ModelPort for FakePort {
    fn embed(
        &self,
        _card: &ModelCard,
        _room: Room,
        _inputs: &[String],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, Error>> + Send {
        future::ready(Err(Error::InvalidAnswer {
            reason: "embedding is unused by this fake".to_owned(),
        }))
    }

    fn rerank(
        &self,
        _card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        self.calls.lock().unwrap().push(RerankCall {
            room,
            query: query.to_owned(),
            documents: documents.to_vec(),
        });
        let reply = self.reply.clone();
        let delay = self.delay;
        async move {
            sleep(delay).await;
            match reply {
                Reply::Scores(scores) => Ok(scores),
                Reply::NoRoom => Err(Error::Unavailable {
                    reason: "no free room".to_owned(),
                }),
            }
        }
    }

    fn tokenize(
        &self,
        _card: &ModelCard,
        _room: Room,
        text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        self.tokenized.lock().unwrap().push(text.to_owned());
        let count = self
            .token_overrides
            .iter()
            .find(|(known_text, _)| known_text == text)
            .map_or_else(|| text.chars().count(), |(_, count)| *count);
        future::ready(Ok(vec![0; count]))
    }

    fn chat(
        &self,
        _card: &ModelCard,
        _room: Room,
        _request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        future::ready(Err(Error::InvalidAnswer {
            reason: "chat is unused by this fake".to_owned(),
        }))
    }
}

pub(super) fn card(role: Role, context_tokens: u32) -> ModelCard {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = env::temp_dir().join(format!(
        "maestro-knowledge-rerank-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let fields = CardFields {
        role,
        router_entry: RouterEntry::parse(if role == Role::Reranker {
            "rerank"
        } else {
            "embed"
        })
        .unwrap(),
        file_digest: Digest::parse(
            "ca56847038f3f329524caec5a86e14865f49918d68094c90dd021a5e67b927f6",
        )
        .unwrap(),
        template_digest: None,
        server_build: "test-build".to_owned(),
        dimensions: (role == Role::Embedder).then(|| NonZeroUsize::new(3).unwrap()),
        limits: Limits {
            context_tokens: NonZeroU32::new(context_tokens).unwrap(),
            output_tokens: None,
        },
        suite_results: vec![SuiteResult {
            suite: "test".to_owned(),
            report: Digest::parse(
                "845e91831319e89c4d656bdb80c278ac09a7230d61e5dfd2e1b1fbb436ac8917",
            )
            .unwrap(),
        }],
    };
    let model = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(root).unwrap();
    model
}

pub(super) fn candidate(id: &str, fusion_score: f64, text: &str) -> Candidate {
    Candidate {
        fused: Fused {
            chunk_id: id.to_owned(),
            score: fusion_score,
            ranks: BTreeMap::new(),
        },
        text: text.to_owned(),
    }
}

fn candidates(count: usize) -> Vec<Candidate> {
    (0..count)
        .map(|index| {
            let id = format!("candidate-{index:02}");
            candidate(&id, 0.25, &id)
        })
        .collect()
}

fn ids(ranked: &[Ranked]) -> Vec<&str> {
    ranked
        .iter()
        .map(|item| item.candidate.fused.chunk_id.as_str())
        .collect()
}

#[tokio::test]
async fn maps_scores_to_document_positions_before_sorting() {
    let port = FakePort::scores(vec![0.1, 0.9, 0.5]);
    let card = card(Role::Reranker, 128);
    let result = rerank(
        "query",
        vec![
            candidate("a", 7.0, "alpha"),
            candidate("b", 6.0, "beta"),
            candidate("c", 5.0, "gamma"),
        ],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(3).unwrap(),
        Duration::from_secs(1),
    )
    .await;

    assert_eq!(result.status, RouteStatus::Ok);
    assert_eq!(ids(&result.ranked), ["b", "c", "a"]);
    assert_eq!(
        result
            .ranked
            .iter()
            .map(|item| item.score)
            .collect::<Vec<_>>(),
        [Some(0.9), Some(0.5), Some(0.1)]
    );
    let calls = port.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].room, Room::Free);
    assert_eq!(calls[0].query, "query");
    assert_eq!(calls[0].documents, ["alpha", "beta", "gamma"]);
}

#[tokio::test]
async fn default_depth_sends_eighty_once_and_keeps_the_unscored_tail() {
    let port = FakePort::scores((0..80).map(f64::from).collect());
    let card = card(Role::Reranker, 128);
    let result = rerank(
        "q",
        candidates(100),
        &Reranker {
            port: &port,
            card: &card,
        },
        DEFAULT_DEPTH,
        Duration::from_secs(1),
    )
    .await;

    let calls = port.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].documents.len(), 80);
    assert_eq!(calls[0].documents[0], "candidate-00");
    assert_eq!(calls[0].documents[79], "candidate-79");
    drop(calls);
    assert_eq!(result.ranked.len(), 100);
    assert_eq!(
        ids(&result.ranked[..80])[..3],
        ["candidate-79", "candidate-78", "candidate-77"]
    );
    assert_eq!(
        ids(&result.ranked[80..]),
        (80..100)
            .map(|i| format!("candidate-{i:02}"))
            .collect::<Vec<_>>()
    );
    assert!(result.ranked[..80].iter().all(|item| item.score.is_some()));
    assert!(result.ranked[80..].iter().all(|item| item.score.is_none()));
}

#[tokio::test]
async fn long_text_is_windowed_without_loss_and_short_text_skips_tokenization() {
    let port = FakePort::scores(vec![0.2, 0.9, 0.4]);
    let card = card(Role::Reranker, 21);
    let result = rerank(
        "q",
        vec![candidate("long", 1.0, "ab cd ef")],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;

    assert_eq!(result.ranked[0].score, Some(0.9));
    {
        let calls = port.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].documents, ["ab ", "cd ", "ef"]);
        assert_eq!(calls[0].documents.concat(), "ab cd ef");
    }
    {
        let tokenized = port.tokenized.lock().unwrap();
        assert!(tokenized.iter().any(|text| text == "q"));
        assert!(tokenized.iter().any(|text| text == "ab cd ef"));
    }

    let port = FakePort::scores(vec![0.4]);
    let result = rerank(
        "q",
        vec![candidate("short", 1.0, "tiny")],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(result.ranked[0].score, Some(0.4));
    assert!(port.tokenized.lock().unwrap().is_empty());
}

fn assert_unavailable(result: &Reranked, expected_ids: &[&str]) -> String {
    assert_eq!(ids(&result.ranked), expected_ids);
    assert!(result.ranked.iter().all(|item| item.score.is_none()));
    let RouteStatus::Unavailable(reason) = &result.status else {
        panic!("expected unavailable, got {:?}", result.status);
    };
    assert!(!reason.trim().is_empty());
    assert_eq!(
        serde_json::to_value(&result.status).unwrap(),
        serde_json::json!({"unavailable": reason})
    );
    reason.clone()
}

#[tokio::test]
async fn unicode_windows_preserve_character_boundaries() {
    let port = FakePort::scores(vec![0.2, 0.9]);
    let card = card(Role::Reranker, 21);
    let result = rerank(
        "q",
        vec![candidate("unicode", 1.0, "éé éé")],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;

    let calls = port.calls.lock().unwrap();
    assert_eq!(calls[0].documents, ["éé ", "éé"]);
    assert_eq!(calls[0].documents.concat(), "éé éé");
    assert_eq!(result.ranked[0].score, Some(0.9));
}

#[tokio::test(start_paused = true)]
async fn every_failure_falls_back_in_fused_order_with_a_reason() {
    let reranker_card = card(Role::Reranker, 128);
    let candidates = || {
        vec![
            candidate("first", 2.0, "one"),
            candidate("second", 1.0, "two"),
        ]
    };
    let depth = NonZeroUsize::new(2).unwrap();

    let port = FakePort::new(Reply::NoRoom, Duration::ZERO);
    let result = rerank(
        "q",
        candidates(),
        &Reranker {
            port: &port,
            card: &reranker_card,
        },
        depth,
        Duration::from_secs(1),
    )
    .await;
    assert_unavailable(&result, &["first", "second"]);

    let port = FakePort::scores(vec![1.0, 0.0]);
    let embedder_card = card(Role::Embedder, 128);
    let result = rerank(
        "q",
        candidates(),
        &Reranker {
            port: &port,
            card: &embedder_card,
        },
        depth,
        Duration::from_secs(1),
    )
    .await;
    assert_unavailable(&result, &["first", "second"]);
    assert!(port.calls.lock().unwrap().is_empty());

    let port = FakePort::delayed_scores(vec![1.0, 0.0], Duration::from_millis(25));
    let result = rerank(
        "q",
        candidates(),
        &Reranker {
            port: &port,
            card: &reranker_card,
        },
        depth,
        Duration::from_millis(1),
    )
    .await;
    let reason = assert_unavailable(&result, &["first", "second"]);
    assert_eq!(reason, "rerank timed out after 1 ms");

    for scores in [vec![1.0], vec![f64::NAN, 0.0]] {
        let port = FakePort::scores(scores);
        let result = rerank(
            "q",
            candidates(),
            &Reranker {
                port: &port,
                card: &reranker_card,
            },
            depth,
            Duration::from_secs(1),
        )
        .await;
        assert_unavailable(&result, &["first", "second"]);
    }
}

#[tokio::test]
async fn equal_rerank_scores_keep_fused_order_and_scores_separate() {
    let port = FakePort::scores(vec![0.0, -0.0, 0.0]);
    let card = card(Role::Reranker, 128);
    let result = rerank(
        "q",
        vec![
            candidate("early", 0.01, "a"),
            candidate("middle", 100.0, "b"),
            candidate("late", -2.0, "c"),
        ],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(3).unwrap(),
        Duration::from_secs(1),
    )
    .await;

    assert_eq!(ids(&result.ranked), ["early", "middle", "late"]);
    assert_eq!(
        result
            .ranked
            .iter()
            .map(|item| item.score)
            .collect::<Vec<_>>(),
        [Some(0.0), Some(-0.0), Some(0.0)]
    );
    assert_eq!(
        result
            .ranked
            .iter()
            .map(|item| item.candidate.fused.score)
            .collect::<Vec<_>>(),
        [0.01, 100.0, -2.0]
    );
}
