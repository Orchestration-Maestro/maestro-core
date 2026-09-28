//! The manifest: what it holds, where its paths lead, and each refusal.

use super::{
    super::manifest::Manifest,
    support::{RERANKER, rung},
};
use crate::failure::Failure;
use maestro_knowledge::search::SearchConfiguration;
use serde_json::{Value, json};
use std::{env, fs, num::NonZeroU32, path::Path, process};

/// A manifest of the rungs `r0` and `r1`.
fn manifest() -> Value {
    json!({
        "schema": "maestro-ladder-manifest/1",
        "suite": "suite.jsonl",
        "collection": "docs",
        "warm_ups": 5,
        "output": "out",
        "rungs": [
            {
                "name": "r0",
                "configuration": {
                    "routes": {
                        "dense": false, "lexical": true, "identifier": true, "structured": false,
                    },
                    "rrf_k": 60,
                    "weights": {"dense": 1.0, "lexical": 1.0, "identifier": 1.0, "structured": 1.0},
                    "rerank": null,
                },
                "ask": false,
            },
            serde_json::to_value(rung("r1")).unwrap(),
        ],
    })
}

/// The manifest `value`, parsed from the directory `/ladder`.
fn parse(value: &Value) -> Result<Manifest, Failure> {
    Manifest::parse(&value.to_string(), Path::new("/ladder"))
}

/// The reason `value` is refused.
fn refusal(value: &Value) -> String {
    match parse(value) {
        Err(Failure::Refused(reason)) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_manifest_holds_its_rungs_and_resolves_its_paths_from_its_directory() {
    let manifest = parse(&manifest()).unwrap();

    assert_eq!(manifest.suite, Path::new("/ladder/suite.jsonl"));
    assert_eq!(manifest.output, Path::new("/ladder/out"));
    assert_eq!(manifest.collection, "docs");
    assert_eq!(manifest.warm_ups, 5);
    assert_eq!(manifest.rungs[1], rung("r1"));
    assert_eq!(
        manifest.rungs[0].configuration.search(),
        SearchConfiguration {
            dense_enabled: false,
            structured_enabled: false,
            rerank_enabled: false,
            ..SearchConfiguration::default()
        }
    );
    assert_eq!(manifest.rungs[0].configuration.reranker().unwrap(), None);
}

#[test]
fn a_rungs_configuration_sets_every_knob_of_search() {
    let configuration = rung("r1").configuration;
    let search = configuration.search();

    assert_eq!(
        search,
        SearchConfiguration {
            dense_enabled: true,
            lexical_enabled: true,
            identifier_enabled: true,
            structured_enabled: false,
            rrf_k: NonZeroU32::new(20).unwrap(),
            dense_weight: 2.0,
            lexical_weight: 1.0,
            identifier_weight: 1.0,
            structured_weight: 1.0,
            rerank_enabled: true,
            rerank_depth: search.rerank_depth,
        }
    );
    assert_eq!(search.rerank_depth.get(), 30);
    assert_eq!(
        configuration.reranker().unwrap().unwrap().as_str(),
        RERANKER
    );
}

#[test]
fn each_malformed_manifest_is_refused() {
    let cases: [(&str, Value, &str); 5] = [
        ("/schema", json!("maestro-ladder-manifest/2"), "schema"),
        ("/collection", json!(" "), "no collection"),
        ("/rungs", json!([]), "no rung"),
        ("/rungs/1/name", json!("r0"), "given twice"),
        ("/rungs/1/name", json!("R 1"), "rung name"),
    ];
    for (pointer, value, reason) in cases {
        let mut changed = manifest();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            refusal(&changed).contains(reason),
            "{pointer}: {}",
            refusal(&changed)
        );
    }
}

#[test]
fn each_rung_search_would_refuse_is_refused() {
    let routes =
        json!({"dense": false, "lexical": false, "identifier": false, "structured": false});
    let cases: [(&str, Value, &str); 5] = [
        ("/rungs/0/configuration/routes", routes, "runs no route"),
        (
            "/rungs/0/configuration/weights/lexical",
            json!(-1.0),
            "weight",
        ),
        (
            "/rungs/1/configuration/rerank/depth",
            json!(121),
            "more than 120",
        ),
        (
            "/rungs/1/configuration/rerank/card",
            json!("abc"),
            "SHA-256",
        ),
        (
            "/rungs/0/configuration/rrf_k",
            json!(0),
            "not maestro-ladder-manifest/1",
        ),
    ];
    for (pointer, value, reason) in cases {
        let mut changed = manifest();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            refusal(&changed).contains(reason),
            "{pointer}: {}",
            refusal(&changed)
        );
    }
}

#[test]
fn unknown_and_missing_keys_are_refused() {
    let mut unknown = manifest();
    unknown["seed"] = json!(1);
    let mut missing = manifest();
    missing["rungs"][0].as_object_mut().unwrap().remove("ask");

    assert!(refusal(&unknown).contains("unknown field `seed`"));
    assert!(refusal(&missing).contains("missing field `ask`"));
    assert!(refusal(&json!([1])).contains("not maestro-ladder-manifest/1"));
}

#[test]
fn an_output_directory_that_holds_files_is_refused() {
    let root = env::temp_dir().join(format!("maestro-ladder-manifest-{}", process::id()));
    fs::create_dir_all(root.join("out")).unwrap();
    let path = root.join("manifest.json");
    fs::write(&path, manifest().to_string()).unwrap();

    assert!(Manifest::read(&path).is_ok());
    fs::write(root.join("out/ladder.md"), "").unwrap();
    assert!(matches!(
        Manifest::read(&path),
        Err(Failure::Refused(reason)) if reason.contains("already holds files")
    ));
    assert!(matches!(
        Manifest::read(&root.join("absent.json")),
        Err(Failure::Refused(reason)) if reason.contains("cannot read the manifest")
    ));
    fs::remove_dir_all(root).unwrap();
}
