//! A rung's question parts: the manifest's knob and glossary pin, the
//! pinned glossary the engine binds, and what the reports record.

use super::{
    super::{
        engine::KernelEngine,
        manifest::Manifest,
        question_parts::{PartsRow, glossary_binding, load_glossary, part_counts},
        runner::Engine as _,
    },
    support::{rung, rung_json, suite},
};
use crate::{
    failure::Failure,
    knowledge::operations::{ask::tests::register_card, tests::Scratch},
};
use maestro_kernel::{
    artifact::Digest,
    evidence::RouteStatus,
    gateway::{Role, RouterClient, Url},
};
use maestro_knowledge::{
    index::Qdrant,
    search::{MAX_GLOSSARY_BYTES, PartRecord, PartsRecord, QuestionParts, Unsplit},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// An address where nothing listens.
const NOWHERE: &str = "http://127.0.0.1:1";
/// A glossary the tests bind.
const GLOSSARY: &str = r#"{"schema": "maestro-glossary/1", "entries": [
    {"id": "G1", "phrases": ["third failure"], "add": ["NumberOfFailures"]}
]}"#;

/// A manifest of one rung, `r1`, whose configuration has `settings` too.
fn manifest(settings: &Value) -> Value {
    let mut rung = rung_json("r1");
    for (key, value) in settings.as_object().unwrap() {
        rung["configuration"][key] = value.clone();
    }
    json!({
        "schema": "maestro-ladder-manifest/1",
        "suite": "suite.jsonl",
        "collection": "docs",
        "warm_ups": 0,
        "output": "out",
        "rungs": [rung],
    })
}

/// The manifest `value`, parsed, or why it is refused.
fn parse(value: &Value) -> Result<Manifest, String> {
    Manifest::parse(&value.to_string(), Path::new("/ladder")).map_err(|error| format!("{error:?}"))
}

#[test]
fn question_parts_are_off_unless_a_rung_splits_and_the_report_echoes_them() {
    let off = parse(&manifest(&json!({}))).unwrap();
    let configuration = &off.rungs[0].configuration;
    assert_eq!(configuration.search().question_parts, QuestionParts::Off);
    let echoed = serde_json::to_value(configuration).unwrap();
    assert!(echoed.get("question_parts").is_none());
    assert!(echoed.get("glossary").is_none());
    let pin = "a".repeat(64);
    let split = parse(&manifest(
        &json!({"question_parts": "split", "glossary": pin}),
    ))
    .unwrap();
    let configuration = &split.rungs[0].configuration;
    assert_eq!(configuration.search().question_parts, QuestionParts::Split);
    let echoed = serde_json::to_value(configuration).unwrap();
    assert_eq!(echoed["question_parts"], "split");
    assert_eq!(echoed["glossary"], pin);
}

#[test]
fn a_glossary_pin_needs_split_parts_and_a_digest() {
    for (settings, reason) in [
        (
            json!({"glossary": "a".repeat(64)}),
            "question parts are off",
        ),
        (
            json!({"question_parts": "split", "glossary": "latest"}),
            "not a SHA-256 digest",
        ),
        (json!({"question_parts": "sometimes"}), "unknown variant"),
    ] {
        let error = parse(&manifest(&settings)).unwrap_err();
        assert!(error.contains(reason), "{settings}: {error}");
    }
}

#[test]
fn a_rung_runs_with_the_bound_glossary_only_when_it_pins_its_digest() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let (_, reranker) = register_card(&kernel, "collection", Role::Reranker, "rerank", b"r");
    let mut split = rung("r0");
    split.configuration.routes.dense = false;
    split.configuration.rerank.as_mut().unwrap().card = reranker.digest().as_str().to_owned();
    split.configuration.question_parts = QuestionParts::Split;
    let digest = Digest::of(GLOSSARY.as_bytes());
    split.configuration.glossary = Some(digest.as_str().to_owned());
    let port = RouterClient::new(Url::parse(NOWHERE).unwrap()).unwrap();
    let mut engine =
        KernelEngine::new(&kernel, "collection", port, Qdrant::new(NOWHERE).unwrap()).unwrap();
    let refusal = format!("{:?}", engine.provenance(&split).unwrap_err());
    assert!(refusal.contains("none is bound"), "{refusal}");

    let path = kernel.config_dir.join("glossary.json");
    fs::write(&path, GLOSSARY).unwrap();
    fs::write(
        kernel.config_dir.join("bindings.toml"),
        format!("glossary_collection = '{}'\n", path.display()),
    )
    .unwrap();
    let (start, _) = engine.start(&split, &suite(0, 1)).unwrap();
    assert_eq!(start.glossary.as_deref(), Some(digest.as_str()));
    assert!(engine.search_context().unwrap().part_bridge.is_some());

    let mut unpinned = split.clone();
    unpinned.configuration.glossary = None;
    engine.start(&unpinned, &suite(0, 1)).unwrap();
    assert!(engine.search_context().unwrap().part_bridge.is_none());
    assert_eq!(engine.provenance(&unpinned).unwrap().glossary, None);

    let mut stale = split.clone();
    stale.configuration.glossary = Some("b".repeat(64));
    let refusal = format!("{:?}", engine.provenance(&stale).unwrap_err());
    assert!(refusal.contains("not the rung's pin"), "{refusal}");
}

#[test]
fn rows_record_parts_without_their_words_and_the_report_counts_splits() {
    let split = PartsRecord {
        whole: Some("chunk-0".to_owned()),
        parts: vec![PartRecord {
            text: "private words".to_owned(),
            bridge: "private terms".to_owned(),
            bridge_status: Some(RouteStatus::Ok),
            status: RouteStatus::Unavailable("part_ranked_nothing".to_owned()),
            best: Some("chunk-1".to_owned()),
        }],
        unsplit: Some(Unsplit::PartCap),
    };
    let row = serde_json::to_value(PartsRow::new(&split)).unwrap();
    assert_eq!(
        row,
        json!({
            "unsplit": "part_cap",
            "whole": "chunk-0",
            "parts": [{
                "status": {"unavailable": "part_ranked_nothing"},
                "bridge": "ok",
                "best": "chunk-1",
            }],
        })
    );
    assert!(!row.to_string().contains("private"));
    let whole = |unsplit| PartsRecord {
        whole: None,
        parts: Vec::new(),
        unsplit,
    };
    let records = [
        split.clone(),
        whole(Some(Unsplit::NoRelation)),
        whole(Some(Unsplit::NoRelation)),
        whole(Some(Unsplit::ShortPart)),
    ];
    assert_eq!(
        part_counts(records.iter()),
        [
            ("no_relation_phrase", 2),
            ("part_too_short", 1),
            ("split", 1)
        ]
        .into_iter()
        .collect()
    );
    assert!(part_counts([].iter()).is_empty());
}

/// A fresh configuration directory for one test.
fn config() -> PathBuf {
    maestro_test_scratch::scratch_directory().unwrap()
}

/// Binds the glossary of `collection` to `text` in `directory`, written when
/// given.
fn bind(directory: &Path, collection: &str, text: Option<&str>) {
    let path = directory.join("glossary.json");
    if let Some(text) = text {
        fs::write(&path, text).unwrap();
    }
    fs::write(
        directory.join("bindings.toml"),
        format!("{} = '{}'\n", glossary_binding(collection), path.display()),
    )
    .unwrap();
}

#[test]
fn a_collection_without_a_binding_has_no_glossary() {
    let directory = config();
    assert!(load_glossary(&directory, "ctm").unwrap().is_none());
    bind(&directory, "other", Some(GLOSSARY));
    assert!(load_glossary(&directory, "ctm").unwrap().is_none());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_bound_glossary_loads_with_its_digest() {
    let directory = config();
    bind(&directory, "ctm", Some(GLOSSARY));
    let glossary = load_glossary(&directory, "ctm").unwrap().unwrap();
    assert_eq!(glossary.digest(), &Digest::of(GLOSSARY.as_bytes()));
    assert_eq!(glossary.bridge("the third failure"), ["NumberOfFailures"]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_missing_oversized_or_invalid_glossary_is_refused() {
    let directory = config();
    bind(&directory, "ctm", None);
    assert!(matches!(
        load_glossary(&directory, "ctm"),
        Err(Failure::Refused(_))
    ));
    bind(&directory, "ctm", Some("{}"));
    assert!(matches!(
        load_glossary(&directory, "ctm"),
        Err(Failure::Refused(_))
    ));
    let oversized = format!("{GLOSSARY}{}", " ".repeat(MAX_GLOSSARY_BYTES));
    bind(&directory, "ctm", Some(&oversized));
    assert!(matches!(
        load_glossary(&directory, "ctm"),
        Err(Failure::Refused(message)) if message.contains("over")
    ));
    fs::write(
        directory.join("bindings.toml"),
        "glossary_ctm = 'relative'\n",
    )
    .unwrap();
    assert!(matches!(
        load_glossary(&directory, "ctm"),
        Err(Failure::Refused(_))
    ));
    fs::remove_dir_all(directory).unwrap();
}
