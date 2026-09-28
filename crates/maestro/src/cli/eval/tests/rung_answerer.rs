//! A rung's answerer and prompt: `ask.card` names a registered answerer card
//! by digest, `ask.prompt` may name a private prompt file read with the
//! manifest, and the rung's provenance holds both.

use super::{
    super::{
        engine::KernelEngine,
        manifest::{AskSettings, Manifest, Rung},
        reports::RungReport,
        rung_prompt::RungPrompt,
        runner::Engine as _,
    },
    ask_settings::manifest,
    reports::{BINARY, runs, to_json},
    support::{rung, suite},
};
use crate::{
    failure::Failure,
    kernel::Kernel,
    knowledge::operations::{
        ask::tests::{register_card, register_reasoning_answerer},
        tests::Scratch,
    },
};
use maestro_kernel::{
    artifact::Digest,
    document::Collection,
    gateway::{ModelCard, Role, RouterClient, Url},
};
use maestro_knowledge::{
    answer::{AnswerPrompt, PromptText, PromptVersion},
    index::Qdrant,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// An address where nothing listens.
const NOWHERE: &str = "http://127.0.0.1:1";

/// A prompt file whose user text holds the `{data}` slot once.
const PROMPT: &str = r#"{"system": "Réponds précisément.", "user": "Données :\n{data}"}"#;

/// A new directory holding `prompt.json` with `text`.
fn prompt_directory(text: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = env::temp_dir().join(format!(
        "maestro-ladder-prompt-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("prompt.json"), text).unwrap();
    root
}

/// The settings of the rung whose `ask` is `ask`, its manifest read from
/// `root`.
fn parsed_in(root: &Path, ask: &Value) -> Result<AskSettings, Failure> {
    Manifest::parse(&manifest(ask).to_string(), root)
        .map(|manifest| manifest.rungs[0].ask.clone().unwrap())
}

/// The reason a rung whose prompt file holds `text` is refused.
fn file_refusal(text: &str) -> String {
    let root = prompt_directory(text);
    let refused = parsed_in(&root, &json!({"prompt": {"file": "prompt.json"}}));
    fs::remove_dir_all(root).unwrap();
    match refused {
        Err(Failure::Refused(reason)) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_prompt_file_is_read_from_the_manifests_directory_with_its_digest() {
    let root = prompt_directory(PROMPT);

    let settings = parsed_in(&root, &json!({"prompt": {"file": "prompt.json"}})).unwrap();

    fs::remove_dir_all(&root).unwrap();
    let RungPrompt::File(file) = &settings.prompt else {
        panic!("expected a prompt file, got {:?}", settings.prompt);
    };
    assert_eq!(file.file, root.join("prompt.json"));
    assert_eq!(
        settings.prompt.digest(),
        Some(Digest::of(PROMPT.as_bytes()).as_str().to_owned())
    );
    assert_eq!(
        settings.prompt.answer_prompt(),
        Some(AnswerPrompt::Text(
            PromptText::new(
                "Réponds précisément.".to_owned(),
                "Données :\n{data}".to_owned()
            )
            .unwrap()
        ))
    );
    assert_eq!(settings.prompt.name(), "file");
}

#[test]
fn a_prompt_file_without_the_slot_or_with_it_twice_or_unreadable_is_refused() {
    let none = file_refusal(r#"{"system": "s", "user": "no slot"}"#);
    let twice = file_refusal(r#"{"system": "s", "user": "{data} {data}"}"#);
    let unknown = file_refusal(r#"{"system": "s", "user": "{data}", "seed": 1}"#);
    let root = prompt_directory(PROMPT);
    let absent = parsed_in(&root, &json!({"prompt": {"file": "absent.json"}}));
    fs::remove_dir_all(root).unwrap();

    for reason in [&none, &twice] {
        assert!(
            reason.contains(concat!(
                "the rung `r0`'s prompt file is refused: invalid ask request: ",
                "the prompt's user text must hold the {data} slot exactly once"
            )),
            "{reason}"
        );
    }
    assert!(unknown.contains("unknown field `seed`"), "{unknown}");
    assert!(
        matches!(
            &absent,
            Err(Failure::Refused(reason))
                if reason.contains("the rung `r0`'s prompt file cannot be read")
        ),
        "{absent:?}"
    );
}

#[test]
fn a_version_prompt_has_no_digest_and_an_unread_file_no_prompt() {
    let v2 = RungPrompt::Version(PromptVersion::V2);
    let unread: RungPrompt = serde_json::from_value(json!({"file": "prompt.json"})).unwrap();

    assert_eq!(v2.digest(), None);
    assert_eq!(v2.answer_prompt(), Some(PromptVersion::V2.into()));
    assert_eq!(v2.name(), "v2");
    assert_eq!(unread.digest(), None);
    assert_eq!(unread.answer_prompt(), None);
    assert_eq!(
        serde_json::to_value(&unread).unwrap(),
        json!({"file": "prompt.json"})
    );
}

#[test]
fn a_card_that_is_not_a_digest_is_refused() {
    let root = prompt_directory(PROMPT);
    let refused = parsed_in(&root, &json!({"card": "qwen3-4b"}));
    let digest = "2".repeat(64);
    let named = parsed_in(&root, &json!({"card": digest})).unwrap();
    fs::remove_dir_all(root).unwrap();

    assert!(
        matches!(
            &refused,
            Err(Failure::Refused(reason))
                if reason == "a rung's answerer card is not a SHA-256 digest"
        ),
        "{refused:?}"
    );
    assert_eq!(named.card.as_deref(), Some(digest.as_str()));
    assert_eq!(
        named.answerer_card().unwrap(),
        Some(Digest::parse(&digest).unwrap())
    );
    assert_eq!(AskSettings::default().answerer_card().unwrap(), None);
}

/// A rung that runs only lexical search and asks with `settings`.
fn asking(settings: AskSettings) -> Rung {
    let mut candidate = rung("r0");
    candidate.configuration.routes.dense = false;
    candidate.configuration.rerank = None;
    candidate.ask = Some(settings);
    candidate
}

/// The engine of `kernel` with the router and the search service down.
fn engine(kernel: &Kernel) -> KernelEngine<'_> {
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    KernelEngine::new(kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap()
}

#[test]
fn a_named_card_is_the_rungs_answerer_and_its_router_entry_is_asked() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, thinking) = register_reasoning_answerer(&kernel, b"thinking", true);
    let (_, bigger) = register_card(&kernel, "collection", Role::Answerer, "qwen3-8b", b"big");
    let (_, default) = register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");
    let engine = engine(&kernel);
    let named = |card: &ModelCard| AskSettings {
        card: Some(card.digest().as_str().to_owned()),
        ..AskSettings::default()
    };

    let thinking_rung = engine.provenance(&asking(named(&thinking))).unwrap();
    let bigger_rung = engine.provenance(&asking(named(&bigger))).unwrap();
    let default_rung = engine.provenance(&asking(AskSettings::default())).unwrap();

    assert_eq!(
        thinking_rung.answerer.as_deref(),
        Some(thinking.digest().as_str())
    );
    assert_eq!(
        bigger_rung.answerer.as_deref(),
        Some(bigger.digest().as_str())
    );
    assert_eq!(
        default_rung.answerer.as_deref(),
        Some(default.digest().as_str())
    );
    assert!(thinking_rung.matches(&default_rung));
    let big = engine.answerer(&asking(named(&bigger))).unwrap();
    let (request, prompt) = engine
        .ask_call("question", &named(&bigger), big.as_ref())
        .unwrap();
    assert_eq!(request.model, "qwen3-8b");
    assert_eq!(prompt, AnswerPrompt::Version(PromptVersion::V2));
}

#[test]
fn a_named_card_of_another_role_collection_or_unregistered_is_refused() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, reranker) = register_card(&kernel, "collection", Role::Reranker, "rerank", b"r");
    kernel
        .database
        .record_collection(&Collection {
            id: "other".to_owned(),
            title: "Other collection".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    let (_, elsewhere) = register_card(&kernel, "other", Role::Answerer, "qwen3-4b", b"o");
    let engine = engine(&kernel);
    let refusal = |digest: &str| {
        let settings = AskSettings {
            card: Some(digest.to_owned()),
            ..AskSettings::default()
        };
        match engine.provenance(&asking(settings)) {
            Err(Failure::Refused(reason)) => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    };

    assert_eq!(
        refusal(reranker.digest().as_str()),
        "a rung's answerer card does not have the answerer role"
    );
    assert_eq!(
        refusal(elsewhere.digest().as_str()),
        "a rung's answerer card is not registered in the collection"
    );
    assert_eq!(
        refusal(&"3".repeat(64)),
        "a rung's answerer card is not registered in the collection"
    );
}

#[test]
fn a_rung_asking_for_more_output_tokens_than_its_answerer_card_allows_is_refused() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");
    let engine = engine(&kernel);
    let output = |tokens: u32| {
        engine.provenance(&asking(AskSettings {
            output_tokens: Some(tokens),
            ..AskSettings::default()
        }))
    };

    assert!(output(1024).is_ok());
    assert!(matches!(
        output(1025),
        Err(Failure::Refused(reason))
            if reason == "the rung `r0` asks for more output tokens than its answerer card allows"
    ));
}

#[test]
fn the_provenance_holds_the_prompt_files_digest() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    register_card(&kernel, "collection", Role::Answerer, "qwen3-4b", b"a");
    let root = prompt_directory(PROMPT);
    let from_file = parsed_in(&root, &json!({"prompt": {"file": "prompt.json"}})).unwrap();
    fs::remove_dir_all(root).unwrap();
    let engine = engine(&kernel);

    let file_rung = engine.provenance(&asking(from_file)).unwrap();
    let v2_rung = engine
        .provenance(&asking(AskSettings {
            prompt: RungPrompt::Version(PromptVersion::V2),
            ..AskSettings::default()
        }))
        .unwrap();
    let mut quiet = asking(AskSettings::default());
    quiet.ask = None;

    assert_eq!(
        file_rung.prompt,
        Some(Digest::of(PROMPT.as_bytes()).as_str().to_owned())
    );
    assert_eq!(v2_rung.prompt, None);
    assert_eq!(engine.provenance(&quiet).unwrap().prompt, None);
    assert!(file_rung.matches(&v2_rung));
}

#[test]
fn an_unread_prompt_file_gives_no_ask_call() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let unread = AskSettings {
        prompt: serde_json::from_value(json!({"file": "prompt.json"})).unwrap(),
        ..AskSettings::default()
    };

    assert_eq!(engine(&kernel).ask_call("question", &unread, None), None);
}

#[test]
fn a_rung_report_names_its_prompt_file_and_its_digest() {
    let root = prompt_directory(PROMPT);
    let from_file = parsed_in(&root, &json!({"prompt": {"file": "prompt.json"}})).unwrap();
    fs::remove_dir_all(root).unwrap();
    let digest = from_file.prompt.digest();
    let mut runs = runs();
    runs[0].rung.ask = Some(from_file);
    runs[0].start.prompt.clone_from(&digest);
    let suite = suite(2, 1);

    let report = RungReport::new(&runs[0], "docs", &suite.digest, BINARY);

    let markdown = report.to_markdown();
    assert!(
        markdown.contains(&format!("- Prompt file: {}\n", digest.unwrap())),
        "{markdown}"
    );
    assert!(
        markdown.contains("output tokens, prompt file\n"),
        "{markdown}"
    );
    assert_eq!(to_json(&report)["ask_settings"]["prompt"], "file");
}
