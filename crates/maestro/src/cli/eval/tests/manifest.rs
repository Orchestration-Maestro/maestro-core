//! The manifest: what it holds, where its paths lead, and each refusal.

use super::{
    super::{command, manifest::Manifest},
    support::{RERANKER, rung, rung_json},
};
use crate::{cli::output::Output, failure::Failure};
use maestro_kernel::evidence::RequestBudget;
use maestro_knowledge::search::{
    CandidateContext, SearchConfiguration, SectionClassSet, SectionPrior, SourceClassSet,
    SourcePrior, StageWindow,
};
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{fs, num::NonZeroU32, path::Path, time::Duration};

/// A manifest of the rungs `r0` and `r1`.
pub(super) fn manifest() -> Value {
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
                    "min_rerank_score": null,
                },
                "ask": false,
            },
            rung_json("r1"),
        ],
    })
}

/// The manifest `value`, parsed from the directory `/ladder`.
pub(super) fn parse(value: &Value) -> Result<Manifest, Failure> {
    Manifest::parse(&value.to_string(), Path::new("/ladder"))
}

/// The reason `value` is refused.
pub(super) fn refusal(value: &Value) -> String {
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
    let mut configuration = rung("r1").configuration;
    configuration.min_rerank_score = Some(0.25);
    configuration.stage_window_ms = NonZeroU32::new(250);
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
            min_rerank_score: Some(0.25),
            stage_window: StageWindow::Fixed(Duration::from_millis(250)),
            ..SearchConfiguration::default()
        }
    );
    assert_eq!(search.rerank_depth.get(), 30);
    assert_eq!(
        configuration.reranker().unwrap().unwrap().as_str(),
        RERANKER
    );
}

#[test]
fn a_rung_may_fix_the_route_window_for_an_experiment() {
    let mut value = manifest();
    value["rungs"][0]["configuration"]["stage_window_ms"] = json!(300);
    let manifest = parse(&value).unwrap();

    assert_eq!(
        manifest.rungs[0].configuration.search().stage_window,
        StageWindow::Fixed(Duration::from_millis(300))
    );
    assert_eq!(
        manifest.rungs[1].configuration.search().stage_window,
        StageWindow::Derived
    );
}

#[test]
fn each_malformed_manifest_is_refused() {
    let cases: [(&str, Value, &str); 6] = [
        ("/schema", json!("maestro-ladder-manifest/2"), "schema"),
        (
            "/rungs/1/name",
            json!("ladder"),
            "the rung name `ladder` names the comparison's files",
        ),
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
    let cases: [(&str, Value, &str); 6] = [
        ("/rungs/0/configuration/routes", routes, "runs no route"),
        (
            "/rungs/0/configuration/min_rerank_score",
            json!(0.25),
            "sets a relevance threshold but does not rerank",
        ),
        (
            "/rungs/0/configuration/weights/lexical",
            json!(-1.0),
            "weight",
        ),
        (
            "/rungs/1/configuration/rerank/depth",
            json!(121),
            "cannot exceed its fusion pool",
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
fn a_rung_that_runs_any_one_route_or_reranks_the_most_candidates_is_accepted() {
    for route in ["dense", "lexical", "identifier", "structured"] {
        let mut one_route = manifest();
        let mut routes =
            json!({"dense": false, "lexical": false, "identifier": false, "structured": false});
        routes[route] = json!(true);
        one_route["rungs"][0]["configuration"]["routes"] = routes;
        assert!(
            parse(&one_route).is_ok(),
            "{route}: {}",
            refusal(&one_route)
        );
    }
    let mut deepest = manifest();
    deepest["rungs"][1]["configuration"]["rerank"]["depth"] = json!(120);

    let rerank = parse(&deepest).unwrap().rungs[1]
        .configuration
        .rerank
        .clone();
    assert_eq!(rerank.unwrap().depth.get(), 120);
}

#[test]
fn a_rung_sets_a_relevance_threshold_and_parsing_refuses_one_beyond_f32() {
    let mut threshold = manifest();
    threshold["rungs"][1]["configuration"]["min_rerank_score"] = json!(0.25);
    let manifest_with_threshold = parse(&threshold).unwrap();
    let mut infinite = manifest();
    infinite["rungs"][1]["configuration"]["min_rerank_score"] = json!(1e300);

    assert_eq!(
        manifest_with_threshold.rungs[1]
            .configuration
            .search()
            .min_rerank_score,
        Some(0.25)
    );
    assert!(
        refusal(&infinite).contains("number out of range"),
        "{}",
        refusal(&infinite)
    );
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
    let root = scratch_directory().unwrap();
    fs::create_dir_all(root.join("out")).unwrap();
    let path = root.join("manifest.json");
    fs::write(&path, manifest().to_string()).unwrap();

    assert!(Manifest::read(&path).is_ok());
    fs::write(root.join("out/ladder.md"), "").unwrap();
    assert!(matches!(
        Manifest::read(&path),
        Err(Failure::Refused(reason)) if reason.contains("already holds files")
    ));
    fs::remove_dir_all(root.join("out")).unwrap();
    fs::write(root.join("out"), "").unwrap();
    assert!(matches!(
        Manifest::read(&path),
        Err(Failure::Refused(reason)) if reason == "the manifest's output path is not a directory"
    ));
    assert!(matches!(
        Manifest::read(&root.join("absent.json")),
        Err(Failure::Refused(reason)) if reason.contains("cannot read the manifest")
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_ladder_command_refuses_a_manifest_it_cannot_read_before_any_search() {
    let scratch = scratch_directory().unwrap();

    let refused = command::run(Output::new(true), &scratch.join("absent.json"));

    fs::remove_dir(&scratch).unwrap();
    assert!(matches!(
        refused,
        Err(Failure::Refused(reason)) if reason.contains("cannot read the manifest")
    ));
}

#[test]
fn rank_knobs_round_trip_and_refuse_unknown_or_out_of_range_values() {
    let mut value = manifest();
    let config = &mut value["rungs"][1]["configuration"];
    config["rerank"]["blend"] = json!(0.25);
    config["rerank"]["demotion_cap"] = json!(2);
    config["rerank"]["candidate_context"] = json!({"mode": "bounded_section", "max_bytes": 1500});
    config["section_prior"] = json!({
        "mode": "soft", "weight": 0.5,
        "classes": ["changelog", "release_notes", "conversion"]
    });
    let parsed = parse(&value).unwrap();
    let config = &parsed.rungs[1].configuration;
    assert_eq!(config.search().rerank_blend, Some(0.25));
    assert_eq!(config.search().rerank_demotion_cap, Some(2));
    assert_eq!(
        config.search().candidate_context,
        CandidateContext::BoundedSection { max_bytes: 1500 }
    );
    let SectionPrior::Soft { weight, classes } = config.search().section_prior else {
        panic!("the manifest sets a soft prior");
    };
    assert!((weight - 0.5).abs() < f32::EPSILON);
    let mut expected = SectionClassSet::default();
    for name in ["changelog", "release_notes", "conversion"] {
        assert!(expected.insert(name));
    }
    assert_eq!(classes, expected);
    let encoded = serde_json::to_value(config).unwrap();
    assert_eq!(encoded["rerank"]["candidate_context"]["max_bytes"], 1500);
    assert_eq!(
        encoded["section_prior"]["classes"],
        json!(["changelog", "release_notes", "conversion"])
    );
    for (pointer, invalid) in [
        ("/rungs/1/configuration/rerank/blend", json!(-0.01)),
        ("/rungs/1/configuration/rerank/blend", json!(1.01)),
        ("/rungs/1/configuration/rerank/demotion_cap", json!(65536)),
        (
            "/rungs/1/configuration/rerank/candidate_context/max_bytes",
            json!(0),
        ),
        (
            "/rungs/1/configuration/rerank/candidate_context/max_bytes",
            json!(1501),
        ),
        ("/rungs/1/configuration/section_prior/weight", json!(1.01)),
        (
            "/rungs/1/configuration/section_prior/classes",
            json!(["unknown"]),
        ),
    ] {
        let mut invalid_value = value.clone();
        *invalid_value.pointer_mut(pointer).unwrap() = invalid;
        assert!(parse(&invalid_value).is_err(), "{pointer}");
    }
}

#[test]
fn the_source_prior_defaults_to_official_first_and_is_set_per_rung() {
    let mut value = manifest();
    let parsed = parse(&value).unwrap();
    let default = &parsed.rungs[1].configuration;
    assert_eq!(default.search().source_prior, SourcePrior::default());
    assert!(
        serde_json::to_value(default)
            .unwrap()
            .get("source_prior")
            .is_none()
    );

    value["rungs"][0]["configuration"]["source_prior"] = json!({"mode": "off"});
    value["rungs"][1]["configuration"]["source_prior"] =
        json!({"mode": "soft", "weight": 0.25, "classes": ["community", "third_party"]});
    let parsed = parse(&value).unwrap();
    assert_eq!(
        parsed.rungs[0].configuration.search().source_prior,
        SourcePrior::Off
    );
    let mut classes = SourceClassSet::default();
    assert!(classes.insert("community"));
    assert!(classes.insert("third_party"));
    let config = &parsed.rungs[1].configuration;
    assert_eq!(
        config.search().source_prior,
        SourcePrior::Soft {
            weight: 0.25,
            classes
        }
    );
    assert_eq!(
        serde_json::to_value(config).unwrap()["source_prior"]["classes"],
        json!(["community", "third_party"])
    );
    for (pointer, invalid) in [
        ("/rungs/1/configuration/source_prior/weight", json!(1.01)),
        ("/rungs/1/configuration/source_prior/weight", json!(-0.01)),
        (
            "/rungs/1/configuration/source_prior/classes",
            json!(["vendor"]),
        ),
    ] {
        let mut invalid_value = value.clone();
        *invalid_value.pointer_mut(pointer).unwrap() = invalid;
        assert!(parse(&invalid_value).is_err(), "{pointer}");
    }
}

#[test]
fn the_identifier_noise_guard_is_off_unless_a_rung_turns_it_on() {
    let mut value = manifest();
    let parsed = parse(&value).unwrap();
    let default = &parsed.rungs[1].configuration;
    assert!(!default.search().identifier_noise_guard);
    assert!(
        serde_json::to_value(default)
            .unwrap()
            .get("identifier_noise_guard")
            .is_none()
    );

    value["rungs"][1]["configuration"]["identifier_noise_guard"] = json!(true);
    let parsed = parse(&value).unwrap();
    let guarded = &parsed.rungs[1].configuration;
    assert!(guarded.search().identifier_noise_guard);
    assert!(
        !parsed.rungs[0]
            .configuration
            .search()
            .identifier_noise_guard
    );
    assert_eq!(
        serde_json::to_value(guarded).unwrap()["identifier_noise_guard"],
        json!(true)
    );
}

#[test]
fn route_limits_and_fusion_pool_are_configurable_and_bounded() {
    let mut value = manifest();
    let configuration = &mut value["rungs"][1]["configuration"];
    configuration["routes_limit"] = json!(64);
    configuration["identifier_limit"] = json!(8);
    configuration["fusion_pool"] = json!(40);
    configuration["rerank"]["depth"] = json!(41);
    assert!(refusal(&value).contains("rerank depth cannot exceed its fusion pool"));

    value["rungs"][1]["configuration"]["rerank"]["depth"] = json!(40);
    let parsed = parse(&value).unwrap();
    let search = parsed.rungs[1].configuration.search();
    assert_eq!(search.routes_limit, 64);
    assert_eq!(search.identifier_limit, 8);
    assert_eq!(search.fusion_pool, 40);

    value["rungs"][1]["configuration"]["fusion_pool"] = json!(121);
    assert!(refusal(&value).contains("fusion_pool must be between 1 and 120"));
    value["rungs"][1]["configuration"]["fusion_pool"] = json!(40);
    value["rungs"][1]["configuration"]["routes_limit"] = json!(0);
    assert!(refusal(&value).contains("routes_limit must be between 1 and 120"));
    value["rungs"][1]["configuration"]["routes_limit"] = json!(100);
    for limit in [0, 121] {
        value["rungs"][1]["configuration"]["identifier_limit"] = json!(limit);
        assert!(refusal(&value).contains("identifier_limit must be between 1 and 120"));
    }
}

#[test]
fn search_budget_is_checked_during_manifest_parse() {
    let mut value = manifest();
    let maximum = RequestBudget::MAX_EVIDENCE_BUDGET;
    value["rungs"][0]["search_budget"] =
        json!({"k": 5, "evidence_bytes": maximum + 1, "deadline_ms": 30000});
    assert!(refusal(&value).contains(&format!("over the {maximum}-byte ceiling")));
    value["rungs"][0]["search_budget"] =
        json!({"k": 5, "evidence_bytes": maximum, "deadline_ms": 30000});
    let budget = parse(&value).unwrap().rungs[0].search_budget.unwrap();
    assert_eq!(budget.evidence_bytes, maximum);

    value["rungs"][0]["search_budget"] =
        json!({"k": 5, "evidence_bytes": 6000, "deadline_ms": 30000});
    value["rungs"][0]["ask"] = json!(true);
    assert!(refusal(&value).contains("search_budget requires ask false"));
}

#[test]
fn search_only_parent_chain_settings_are_optional_and_validated() {
    let mut value = manifest();
    value["rungs"][0]["configuration"]["evidence_expansion"] = json!("parent_chain");
    value["rungs"][0]["configuration"]["parent_chain_order"] = json!("largest_fitting_parent");
    assert!(parse(&value).is_ok());
    value["rungs"][0]["configuration"]["evidence_expansion"] = json!("full_section");
    assert!(parse(&value).is_err());

    value["rungs"][0]["ask"] = json!(true);
    value["rungs"][0]["configuration"]["evidence_expansion"] = json!("relevant_blocks");
    value["rungs"][0]["configuration"]
        .as_object_mut()
        .unwrap()
        .remove("parent_chain_order");
    assert!(refusal(&value).contains("search-only evidence settings"));
}
