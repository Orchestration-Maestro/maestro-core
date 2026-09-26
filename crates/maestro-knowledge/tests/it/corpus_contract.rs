//! `maestro-corpus/1`: one line per document parses into typed values; an
//! unknown key, a key given twice at any depth, a path that leaves the
//! manifest's directory, a malformed digest or size and a `source_ref` that is
//! neither a web URL nor the line's own corpus path are refused.
#![cfg(test)]

use maestro_kernel::artifact::Digest;
use maestro_knowledge::corpus::{Entry, Error, Schema};
use serde_json::{Value, json};
use std::error;

/// SHA-256 of `abc`.
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// A line with the seven required keys, on a public topic.
fn line() -> Value {
    json!({
        "schema": "maestro-corpus/1",
        "path": "beds/raised-beds.md",
        "sha256": ABC,
        "bytes": 1832,
        "source_ref": "https://example.org/garden/raised-beds",
        "title": "Building raised beds",
        "source_kind": "guide"
    })
}

/// [`line`] with `key` set to `value`.
fn with(key: &str, value: Value) -> Value {
    let mut changed = line();
    changed[key] = value;
    changed
}

fn parse(text: &str) -> Result<Entry, Error> {
    text.parse()
}

/// The refusal of `text` as JSON the contract does not accept, as displayed.
fn json_refusal(text: &str) -> String {
    match parse(text) {
        Err(refusal @ Error::Json(_)) => refusal.to_string(),
        other => panic!("expected a JSON refusal of {text}, got {other:?}"),
    }
}

/// The `source_ref` refused in `line`, which must be refused for it.
fn source_ref_refusal(line: &Value) -> String {
    match parse(&line.to_string()) {
        Err(Error::SourceRef(refused)) => refused,
        other => panic!("expected a source_ref refusal of {line}, got {other:?}"),
    }
}

#[test]
fn a_line_with_the_required_keys_parses() {
    let entry = parse(&line().to_string()).unwrap();
    assert_eq!(entry.schema, Schema::V1);
    assert_eq!(entry.path.as_str(), "beds/raised-beds.md");
    assert_eq!(entry.sha256, Digest::parse(ABC).unwrap());
    assert_eq!(entry.bytes.get(), 1832);
    assert_eq!(entry.source_ref, "https://example.org/garden/raised-beds");
    assert_eq!(entry.title, "Building raised beds");
    assert_eq!(entry.source_kind, "guide");
    let absent = [
        &entry.set,
        &entry.version,
        &entry.lang,
        &entry.captured_at,
        &entry.product,
        &entry.component,
        &entry.platform,
    ];
    assert!(absent.iter().all(|value| value.is_none()), "{entry:?}");
    assert_eq!((entry.extractor, entry.access), (None, None));
}

#[test]
fn a_line_with_every_key_parses() {
    let extractor = json!({
        "name": "hand-written",
        "version": "1",
        "options": {
            "depth": 2,
            "offset": -3,
            "ratio": 0.5,
            "strict": true,
            "note": null,
            "tags": ["soil", "tab\tseparated"]
        }
    });
    let access = json!({ "visibility": "public", "license": "CC-BY-4.0" });
    let mut full = line();
    for (key, value) in [
        ("set", json!("vegetables")),
        ("version", json!("2026")),
        ("lang", json!("en")),
        ("captured_at", json!("2026-09-13T16:34:20Z")),
        ("product", json!("garden-planner")),
        ("component", json!("beds")),
        ("platform", json!("outdoor")),
        ("extractor", extractor.clone()),
        ("access", access.clone()),
    ] {
        full[key] = value;
    }
    let entry = parse(&full.to_string()).unwrap();
    assert_eq!(entry.set.as_deref(), Some("vegetables"));
    assert_eq!(entry.version.as_deref(), Some("2026"));
    assert_eq!(entry.lang.as_deref(), Some("en"));
    assert_eq!(entry.captured_at.as_deref(), Some("2026-09-13T16:34:20Z"));
    assert_eq!(entry.product.as_deref(), Some("garden-planner"));
    assert_eq!(entry.component.as_deref(), Some("beds"));
    assert_eq!(entry.platform.as_deref(), Some("outdoor"));
    assert_eq!(entry.extractor.map(Value::Object), Some(extractor));
    assert_eq!(entry.access.map(Value::Object), Some(access));
}

#[test]
fn an_unknown_key_is_refused() {
    let reason = json_refusal(&with("season", json!("spring")).to_string());
    assert!(reason.contains("unknown field `season`"), "{reason}");
}

#[test]
fn a_key_given_twice_is_refused_inside_nested_maps_too() {
    let mut nested = with(
        "extractor",
        json!({ "name": "hand-written", "options": { "depth": 2, "tags": [{ "soil": "loam" }] } }),
    );
    nested["access"] = json!({ "license": "CC-BY-4.0" });
    let text = nested.to_string();
    for (once, again, key) in [
        (
            "\"title\":\"Building raised beds\"",
            "\"title\":\"Pruning roses\"",
            "title",
        ),
        ("\"name\":\"hand-written\"", "\"name\":\"scanned\"", "name"),
        ("\"depth\":2", "\"depth\":3", "depth"),
        ("\"soil\":\"loam\"", "\"soil\":\"clay\"", "soil"),
        (
            "\"license\":\"CC-BY-4.0\"",
            "\"license\":\"MIT\"",
            "license",
        ),
    ] {
        assert!(text.contains(once), "{once} is not in {text}");
        let twice = text.replacen(once, &format!("{once},{again}"), 1);
        let reason = json_refusal(&twice);
        assert!(
            reason.contains("duplicate") && reason.contains(&format!("`{key}`")),
            "{again}: {reason}"
        );
    }
}

#[test]
fn an_extractor_or_access_that_is_not_an_object_is_refused() {
    for key in ["extractor", "access"] {
        for value in [json!("hand-written"), json!(["hand-written"]), json!(7)] {
            let reason = json_refusal(&with(key, value.clone()).to_string());
            assert!(
                reason.contains("invalid type") && reason.contains("expected a JSON object"),
                "{key} = {value}: {reason}"
            );
        }
    }
}

#[test]
fn a_line_without_a_required_key_is_refused() {
    for key in [
        "schema",
        "path",
        "sha256",
        "bytes",
        "source_ref",
        "title",
        "source_kind",
    ] {
        let mut partial = line();
        partial.as_object_mut().unwrap().remove(key).unwrap();
        let reason = json_refusal(&partial.to_string());
        assert!(
            reason.contains(&format!("missing field `{key}`")),
            "{key}: {reason}"
        );
    }
}

#[test]
fn the_schema_is_maestro_corpus_1() {
    for schema in ["maestro-corpus/2", "maestro-collection/1", ""] {
        let reason = json_refusal(&with("schema", json!(schema)).to_string());
        assert!(reason.contains("unknown variant"), "{schema}: {reason}");
    }
}

#[test]
fn a_path_that_is_absolute_or_climbs_out_of_the_manifest_directory_is_refused() {
    for path in [
        "",
        "/srv/garden/beds.md",
        "C:/garden/beds.md",
        "c:beds.md",
        "../beds.md",
        "beds/../../beds.md",
        "beds/..",
        "beds\\raised-beds.md",
        "\\\\server\\share\\beds.md",
    ] {
        let reason = json_refusal(&with("path", json!(path)).to_string());
        assert!(reason.contains(&format!("{path:?}")), "{path}: {reason}");
    }
}

#[test]
fn a_path_inside_the_manifest_directory_parses() {
    for path in [
        "beds.md",
        "beds/raised/deep.md",
        "./beds.md",
        "beds..md",
        "..beds/deep.md",
    ] {
        let entry = parse(&with("path", json!(path)).to_string()).unwrap();
        assert_eq!(entry.path.as_str(), path);
    }
}

#[test]
fn sha256_is_64_lowercase_hexadecimal_characters() {
    let upper = ABC.to_uppercase();
    let short = ABC.get(1..).unwrap();
    let long = format!("{ABC}0");
    let not_hex = ABC.replacen('b', "g", 1);
    for digest in [upper.as_str(), short, &long, &not_hex, ""] {
        let reason = json_refusal(&with("sha256", json!(digest)).to_string());
        assert!(reason.contains("SHA-256"), "{digest}: {reason}");
    }
}

#[test]
fn bytes_is_a_positive_whole_number() {
    let text = line().to_string();
    let written = "\"bytes\":1832";
    assert!(text.contains(written), "{text}");
    for bytes in [
        "0",
        "-1",
        "1.5",
        "1832.0",
        "1.832e3",
        "18446744073709551616",
        "1e999",
        "\"1832\"",
        "null",
    ] {
        let changed = text.replacen(written, &format!("\"bytes\":{bytes}"), 1);
        assert!(matches!(parse(&changed), Err(Error::Json(_))), "{bytes}");
    }
    for (bytes, expected) in [("1", 1), ("18446744073709551615", u64::MAX)] {
        let changed = text.replacen(written, &format!("\"bytes\":{bytes}"), 1);
        assert_eq!(parse(&changed).unwrap().bytes.get(), expected);
    }
}

#[test]
fn a_number_out_of_range_is_refused_anywhere_in_the_line() {
    let text = with("extractor", json!({ "version": "1" })).to_string();
    let changed = text.replacen("\"version\":\"1\"", "\"version\":1e999", 1);
    assert_ne!(changed, text);
    let reason = json_refusal(&changed);
    assert!(reason.contains("number out of range"), "{reason}");
}

#[test]
fn source_ref_is_a_web_url_or_the_line_s_own_corpus_path() {
    for source_ref in [
        "https://example.org/garden/raised-beds",
        "http://example.org",
        "https://example.org:8443/beds?page=2#top",
        "corpus-path:beds/raised-beds.md",
    ] {
        let entry = parse(&with("source_ref", json!(source_ref)).to_string()).unwrap();
        assert_eq!(entry.source_ref, source_ref);
    }
}

#[test]
fn a_corpus_path_that_names_another_path_is_refused() {
    for source_ref in [
        "corpus-path:beds/pruning.md",
        "corpus-path:",
        "corpus-path:/beds/raised-beds.md",
        "corpus-path:beds/raised-beds.md ",
    ] {
        let refused = source_ref_refusal(&with("source_ref", json!(source_ref)));
        assert_eq!(refused, source_ref);
    }
}

#[test]
fn a_source_ref_that_is_not_a_web_url_is_refused() {
    for source_ref in [
        "",
        "example.org/garden",
        "ftp://example.org/garden",
        "mailto:gardener@example.org",
        "HTTPS://example.org/garden",
        "https://",
        "https:///garden",
        "https://?page=2",
        "https://example.org/raised beds",
        "https://example.org/\u{7}",
    ] {
        let refused = source_ref_refusal(&with("source_ref", json!(source_ref)));
        assert_eq!(refused, source_ref);
    }
}

#[test]
fn text_that_is_not_one_json_value_is_refused() {
    let text = line().to_string();
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
        refusal.to_string().contains("maestro-corpus/1"),
        "{refusal}"
    );
    assert!(error::Error::source(&refusal).is_some());
    let refusal = parse(&with("source_ref", json!("ftp://example.org")).to_string()).unwrap_err();
    assert!(refusal.to_string().contains("source_ref"), "{refusal}");
    assert!(error::Error::source(&refusal).is_none());
}
