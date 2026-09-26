//! `maestro-collection/1`: a strict declaration parses into typed values; an
//! unknown or repeated key, an unknown value, an object written as an array or
//! a name as an object, a number out of range, a path that leaves its directory
//! and two sources with one id are refused, and so is a manifest whose binding
//! is missing, before any is resolved.
#![cfg(test)]

use maestro_kernel::binding::{self, Bindings};
use maestro_knowledge::collection::{
    Declaration, Error, Schema, SourceKind, Synchronization, Visibility,
};
use serde_json::{Value, json};
use std::{env, error, fmt::Write as _, path::PathBuf};

/// A declaration with every key the contract names, on a public topic.
fn declaration() -> Value {
    json!({
        "schema": "maestro-collection/1",
        "id": "garden",
        "title": "Kitchen garden notes",
        "visibility": "public",
        "profiles": {
            "extraction": "technical-html/1",
            "chunking": "structural-500-700/1",
            "embedding": "embed:winner",
            "sparse": "bm25-en-fr/1"
        },
        "quality": { "ledger": "quality/ledger.jsonl" },
        "sources": [{
            "id": "seed-catalog",
            "kind": "import",
            "sync": "manual",
            "manifest": { "binding": "garden_root", "path": "2026/maestro-corpus.jsonl" }
        }],
        "evals": { "suite": "evals/garden" }
    })
}

/// The declaration with a second source, `orchard-notes`, bound to
/// `orchard_root`.
fn two_sources() -> Value {
    let mut declared = declaration();
    let first = declared["sources"][0].clone();
    let mut second = first.clone();
    second["id"] = json!("orchard-notes");
    second["manifest"] = json!({ "binding": "orchard_root", "path": "notes.jsonl" });
    declared["sources"] = json!([first, second]);
    declared
}

fn parse(text: &str) -> Result<Declaration, Error> {
    text.parse()
}

/// The refusal of `text` as JSON the contract does not accept, as displayed.
fn json_refusal(text: &str) -> String {
    match parse(text) {
        Err(refusal @ Error::Json(_)) => refusal.to_string(),
        other => panic!("expected a JSON refusal of {text}, got {other:?}"),
    }
}

/// The directory [`bindings`] binds `name` to: absolute on every host.
fn root(name: &str) -> PathBuf {
    env::temp_dir().join(name)
}

/// Bindings of `names`, each to its [`root`].
fn bindings(names: &[&str]) -> Bindings {
    let mut text = String::new();
    for name in names {
        writeln!(text, "{name} = '{}'", root(name).display()).unwrap();
    }
    text.parse().unwrap()
}

#[test]
fn a_declaration_parses_into_typed_values() {
    let parsed = parse(&declaration().to_string()).unwrap();
    assert_eq!(parsed.schema, Schema::V1);
    assert_eq!(parsed.id, "garden");
    assert_eq!(parsed.title, "Kitchen garden notes");
    assert_eq!(parsed.visibility, Visibility::Public);
    let profiles = &parsed.profiles;
    assert_eq!(
        [
            profiles.extraction.as_str(),
            &profiles.chunking,
            &profiles.embedding,
            &profiles.sparse
        ],
        [
            "technical-html/1",
            "structural-500-700/1",
            "embed:winner",
            "bm25-en-fr/1"
        ]
    );
    assert_eq!(parsed.quality.ledger.as_str(), "quality/ledger.jsonl");
    assert_eq!(parsed.evals.suite.as_str(), "evals/garden");
    let [source] = parsed.sources.as_slice() else {
        panic!("one source: {:?}", parsed.sources);
    };
    assert_eq!(source.id, "seed-catalog");
    assert_eq!(source.kind, SourceKind::Import);
    assert_eq!(source.sync, Synchronization::Manual);
    assert_eq!(source.manifest.binding, "garden_root");
    assert_eq!(source.manifest.path.as_str(), "2026/maestro-corpus.jsonl");
}

#[test]
fn every_visibility_and_synchronization_policy_parses() {
    for (visibility, expected) in [
        ("public", Visibility::Public),
        ("private", Visibility::Private),
    ] {
        let mut declared = declaration();
        declared["visibility"] = json!(visibility);
        assert_eq!(parse(&declared.to_string()).unwrap().visibility, expected);
    }
    for (sync, expected) in [
        ("one-off", Synchronization::OneOff),
        ("manual", Synchronization::Manual),
        ("watch", Synchronization::Watch),
    ] {
        let mut declared = declaration();
        declared["sources"][0]["sync"] = json!(sync);
        assert_eq!(
            parse(&declared.to_string()).unwrap().sources[0].sync,
            expected
        );
    }
}

#[test]
fn sources_with_distinct_ids_parse_in_their_order() {
    let parsed = parse(&two_sources().to_string()).unwrap();
    let ids: Vec<_> = parsed
        .sources
        .iter()
        .map(|source| source.id.as_str())
        .collect();
    assert_eq!(ids, ["seed-catalog", "orchard-notes"]);
}

#[test]
fn a_value_the_contract_does_not_name_is_refused() {
    for (pointer, value) in [
        ("/schema", "maestro-collection/2"),
        ("/schema", "maestro-corpus/1"),
        ("/visibility", "internal"),
        ("/sources/0/kind", "crawl"),
        ("/sources/0/sync", "hourly"),
    ] {
        let mut declared = declaration();
        *declared.pointer_mut(pointer).unwrap() = json!(value);
        let reason = json_refusal(&declared.to_string());
        assert!(
            reason.contains("unknown variant"),
            "{pointer} = {value}: {reason}"
        );
    }
}

#[test]
fn a_named_value_written_as_an_object_is_refused() {
    for (pointer, value) in [
        ("/schema", "maestro-collection/1"),
        ("/visibility", "public"),
        ("/sources/0/kind", "import"),
        ("/sources/0/sync", "manual"),
    ] {
        let mut declared = declaration();
        *declared.pointer_mut(pointer).unwrap() = json!({ value: null });
        let reason = json_refusal(&declared.to_string());
        assert!(reason.contains("invalid type: map"), "{pointer}: {reason}");
    }
}

#[test]
fn an_object_written_as_an_array_is_refused_at_every_level() {
    let declared = declaration();
    // Each object's values in the order its fields are declared, without keys.
    let whole = json!([
        "maestro-collection/1",
        "garden",
        "Kitchen garden notes",
        "public",
        declared["profiles"],
        declared["quality"],
        declared["sources"],
        declared["evals"]
    ]);
    let profiles = json!([
        "technical-html/1",
        "structural-500-700/1",
        "embed:winner",
        "bm25-en-fr/1"
    ]);
    let source = json!([
        "seed-catalog",
        "import",
        "manual",
        declared["sources"][0]["manifest"]
    ]);
    let manifest = json!(["garden_root", "2026/maestro-corpus.jsonl"]);
    for (pointer, values) in [
        ("", whole),
        ("/profiles", profiles),
        ("/quality", json!(["quality/ledger.jsonl"])),
        ("/sources/0", source),
        ("/sources/0/manifest", manifest),
        ("/evals", json!(["evals/garden"])),
    ] {
        let mut changed = declaration();
        *changed.pointer_mut(pointer).unwrap() = values;
        let reason = json_refusal(&changed.to_string());
        assert!(
            reason.contains("invalid type: sequence, expected a JSON object"),
            "{pointer}: {reason}"
        );
    }
}

#[test]
fn an_unknown_key_is_refused_at_every_level() {
    for pointer in [
        "",
        "/profiles",
        "/quality",
        "/sources/0",
        "/sources/0/manifest",
        "/evals",
    ] {
        let mut declared = declaration();
        let object = declared.pointer_mut(pointer).unwrap();
        object["season"] = json!("spring");
        let reason = json_refusal(&declared.to_string());
        assert!(
            reason.contains("unknown field `season`"),
            "{pointer}: {reason}"
        );
    }
}

#[test]
fn a_key_given_twice_is_refused_at_every_level() {
    let text = declaration().to_string();
    for (once, again) in [
        ("\"id\":\"garden\"", "\"id\":\"orchard\""),
        ("\"sparse\":\"bm25-en-fr/1\"", "\"sparse\":\"bm25-en-fr/1\""),
        (
            "\"ledger\":\"quality/ledger.jsonl\"",
            "\"ledger\":\"ledger.jsonl\"",
        ),
        ("\"sync\":\"manual\"", "\"sync\":\"watch\""),
        (
            "\"binding\":\"garden_root\"",
            "\"binding\":\"orchard_root\"",
        ),
    ] {
        assert!(text.contains(once), "{once} is not in {text}");
        let twice = text.replacen(once, &format!("{once},{again}"), 1);
        let reason = json_refusal(&twice);
        assert!(reason.contains("duplicate field"), "{again}: {reason}");
    }
}

#[test]
fn a_missing_key_is_refused_at_every_level() {
    for (pointer, key) in [
        ("", "sources"),
        ("", "evals"),
        ("/profiles", "sparse"),
        ("/quality", "ledger"),
        ("/sources/0", "manifest"),
        ("/sources/0/manifest", "binding"),
    ] {
        let mut declared = declaration();
        let object = declared.pointer_mut(pointer).unwrap();
        object.as_object_mut().unwrap().remove(key).unwrap();
        let reason = json_refusal(&declared.to_string());
        assert!(
            reason.contains(&format!("missing field `{key}`")),
            "{pointer}/{key}: {reason}"
        );
    }
}

#[test]
fn a_number_out_of_range_is_refused() {
    for number in ["1e999", "-1e999"] {
        let text = declaration()
            .to_string()
            .replacen("\"Kitchen garden notes\"", number, 1);
        let reason = json_refusal(&text);
        assert!(reason.contains("number out of range"), "{number}: {reason}");
    }
}

#[test]
fn text_that_is_not_one_json_value_is_refused() {
    let text = declaration().to_string();
    for refused in [
        String::new(),
        "garden".to_owned(),
        format!("{text} {text}"),
        text.replacen('}', "", 1),
    ] {
        assert!(matches!(parse(&refused), Err(Error::Json(_))), "{refused}");
    }
}

#[test]
fn a_refusal_names_the_contract_and_keeps_its_cause() {
    let refusal = parse("{}").unwrap_err();
    assert!(
        refusal.to_string().contains("maestro-collection/1"),
        "{refusal}"
    );
    assert!(error::Error::source(&refusal).is_some());
}

#[test]
fn a_path_that_is_absolute_or_leaves_its_directory_is_refused() {
    for pointer in [
        "/quality/ledger",
        "/evals/suite",
        "/sources/0/manifest/path",
    ] {
        for path in [
            "",
            "/srv/garden/notes.jsonl",
            "C:/garden/notes.jsonl",
            "c:notes.jsonl",
            "../notes.jsonl",
            "notes/../../notes.jsonl",
            "notes/..",
            "notes\\notes.jsonl",
            "beds/C:/beds.md",
            "beds/c:beds.md",
            "beds.md:notes",
        ] {
            let mut declared = declaration();
            *declared.pointer_mut(pointer).unwrap() = json!(path);
            let reason = json_refusal(&declared.to_string());
            assert!(
                reason.contains(&format!("{path:?}")),
                "{pointer} = {path}: {reason}"
            );
        }
    }
}

#[test]
fn a_path_that_stays_inside_its_directory_parses() {
    for path in [
        "notes.jsonl",
        "2026/spring/notes.jsonl",
        "./notes.jsonl",
        "notes..jsonl",
        "..notes/notes.jsonl",
        "evals/garden/",
    ] {
        let mut declared = declaration();
        declared["evals"]["suite"] = json!(path);
        let parsed = parse(&declared.to_string()).unwrap();
        assert_eq!(parsed.evals.suite.as_str(), path);
    }
}

#[test]
fn two_sources_with_one_id_are_refused() {
    let mut declared = two_sources();
    declared["sources"][1]["id"] = json!("seed-catalog");
    let refusal = parse(&declared.to_string()).unwrap_err();
    assert!(
        matches!(&refusal, Error::DuplicateSource(id) if id == "seed-catalog"),
        "{refusal:?}"
    );
    assert!(refusal.to_string().contains("`seed-catalog`"), "{refusal}");
    assert!(error::Error::source(&refusal).is_none());
}

#[test]
fn every_manifest_resolves_through_its_binding() {
    let parsed = parse(&two_sources().to_string()).unwrap();
    let resolved: Vec<_> = parsed
        .manifest_paths(&bindings(&["garden_root", "orchard_root"]))
        .unwrap()
        .into_iter()
        .map(|(source, path)| (source.id.as_str(), path))
        .collect();
    assert_eq!(
        resolved,
        [
            (
                "seed-catalog",
                root("garden_root")
                    .join("2026")
                    .join("maestro-corpus.jsonl")
            ),
            ("orchard-notes", root("orchard_root").join("notes.jsonl")),
        ]
    );
}

#[test]
fn a_missing_binding_refuses_every_manifest_and_names_it() {
    let parsed = parse(&two_sources().to_string()).unwrap();
    for (bound, missing) in [
        (&["garden_root"], "orchard_root"),
        (&["orchard_root"], "garden_root"),
    ] {
        let refusal = parsed.manifest_paths(&bindings(bound)).unwrap_err();
        assert!(
            matches!(&refusal, binding::Error::Missing(name) if name == missing),
            "{refusal:?}"
        );
    }
}
