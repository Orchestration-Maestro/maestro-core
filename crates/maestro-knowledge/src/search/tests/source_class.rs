//! Source classes: the table adapter, the class vocabulary and the prior.

use super::rerank::candidate;
use crate::search::{
    Ranked, SourceClass, SourceClassSet, SourceClassTable, SourceClassifier, SourceMetadata,
    SourcePrior, rank_policy,
};
use maestro_kernel::artifact::Digest;
use std::{collections::BTreeMap, error};

const TABLE: &str = r#"{
  "schema": "maestro-source-classes/1",
  "rules": [
    {"source_kind": "web", "host": "docs.example.org", "path_prefix": "/kb/",
     "class": "official_kb", "label": "Example KB"},
    {"host": "Docs.Example.org", "class": "official_docs", "label": "Example docs"},
    {"path_prefix": "repos/community/", "class": "community", "label": "Community repo"},
    {"path_prefix": "repos/", "class": "official_code", "label": "Example repo"}
  ]
}"#;

fn table() -> SourceClassTable {
    SourceClassTable::parse(TABLE.as_bytes()).unwrap()
}

fn classify(kind: Option<&str>, source_ref: &str) -> Option<(&'static str, String)> {
    table()
        .classify(&SourceMetadata {
            source_kind: kind,
            source_ref,
        })
        .map(|found| (found.class.name(), found.label))
}

#[test]
fn the_first_matching_rule_names_the_class_and_label() {
    assert_eq!(
        classify(Some("web"), "https://docs.example.org/kb/1"),
        Some(("official_kb", "Example KB".to_owned()))
    );
    assert_eq!(
        classify(Some("other"), "https://docs.example.org/kb/1"),
        Some(("official_docs", "Example docs".to_owned()))
    );
    assert_eq!(
        classify(None, "https://DOCS.example.org"),
        Some(("official_docs", "Example docs".to_owned()))
    );
    assert_eq!(
        classify(None, "corpus-path:repos/community/scale.md"),
        Some(("community", "Community repo".to_owned()))
    );
    assert_eq!(
        classify(None, "corpus-path:repos/tool/readme.md"),
        Some(("official_code", "Example repo".to_owned()))
    );
}

#[test]
fn a_host_rule_reads_the_host_past_a_login_a_port_and_a_query() {
    for source_ref in [
        "https://reader@docs.example.org/page",
        "https://docs.example.org:8443/page",
        "https://docs.example.org?page=1",
        "http://docs.example.org#top",
    ] {
        assert_eq!(
            classify(None, source_ref),
            Some(("official_docs", "Example docs".to_owned())),
            "{source_ref}"
        );
    }
    assert_eq!(
        classify(Some("web"), "https://docs.example.org:8443/kb/1?x=1"),
        Some(("official_kb", "Example KB".to_owned()))
    );
}

#[test]
fn an_unmatched_source_has_no_class() {
    assert_eq!(classify(None, "https://elsewhere.example.org/kb/1"), None);
    assert_eq!(classify(None, "https://example.org/docs.example.org"), None);
    assert_eq!(classify(None, "corpus-path:other/repos/a.md"), None);
}

#[test]
fn the_table_keeps_the_digest_of_its_bytes() {
    assert_eq!(table().digest(), &Digest::of(TABLE.as_bytes()));
}

/// A version-1 table of the rules `rules`, written as JSON.
fn rules(rules: &str) -> String {
    format!(r#"{{"schema": "maestro-source-classes/1", "rules": [{rules}]}}"#)
}

#[test]
fn a_malformed_table_is_refused() {
    for text in [
        r#"{"schema": "maestro-source-classes/2", "rules": []}"#.to_owned(),
        rules(r#"{"class": "vendor", "label": "x"}"#),
        rules(r#"{"class": "community", "label": " "}"#),
        rules(r#"{"class": "community", "label": "x", "url": "y"}"#),
        rules(r#"{"class": "community", "label": "x"}"#),
        rules(r#"{"host": null, "class": "community", "label": "x"}"#),
        rules(r#"{"path_prefix": "", "class": "community", "label": "x"}"#),
        rules(r#"{"host": "", "class": "community", "label": "x"}"#),
        rules(r#"{"source_kind": "", "class": "community", "label": "x"}"#),
        rules(r#"{"host": "a.org", "class": "community", "label": "two\nlines"}"#),
        rules(r#"{"host": "a.org", "path_prefix": "docs/", "class": "community", "label": "x"}"#),
        r#"{"schema": "maestro-source-classes/1", "rules": [], "rules": []}"#.to_owned(),
        r#"{"schema": "maestro-source-classes/1", "rules": []} []"#.to_owned(),
    ] {
        assert!(SourceClassTable::parse(text.as_bytes()).is_err(), "{text}");
    }
    assert!(SourceClassTable::parse(&[0xff]).is_err());
}

#[test]
fn each_refusal_says_why() {
    let reason = |text: &[u8]| {
        let error = SourceClassTable::parse(text).unwrap_err();
        (error.to_string(), error::Error::source(&error).is_some())
    };
    assert_eq!(
        reason(&[0xff]),
        ("the source-class table is not UTF-8".to_owned(), false)
    );
    let (json, has_source) = reason(b"[]");
    assert!(
        json.starts_with("the source-class table is invalid: "),
        "{json}"
    );
    assert!(has_source);
    assert_eq!(
        reason(br#"{"schema": "v2", "rules": []}"#),
        ("unknown source-class table schema \"v2\"".to_owned(), false)
    );
    assert_eq!(
        reason(rules(r#"{"host": "a.org", "class": "vendor", "label": "x"}"#).as_bytes()),
        ("unknown source class \"vendor\"".to_owned(), false)
    );
    assert_eq!(
        reason(rules(r#"{"host": "a.org", "class": "community", "label": ""}"#).as_bytes()),
        ("a source-class label is blank".to_owned(), false)
    );
    assert_eq!(
        reason(rules(r#"{"class": "community", "label": "x"}"#).as_bytes()),
        (
            "a source-class rule names no source_kind, host or path_prefix".to_owned(),
            false
        )
    );
    assert_eq!(
        reason(rules(r#"{"host": "a.org", "class": "community", "label": "a\tb"}"#).as_bytes()),
        (
            "a source-class label holds a control character".to_owned(),
            false
        )
    );
    assert_eq!(
        reason(
            rules(r#"{"host": "a.org", "path_prefix": "x", "class": "community", "label": "x"}"#)
                .as_bytes()
        ),
        (
            "a host rule's path_prefix must start with `/`".to_owned(),
            false
        )
    );
}

#[test]
fn class_names_are_the_vocabulary_and_nothing_else() {
    for name in [
        "official_docs",
        "official_kb",
        "official_code",
        "community",
        "third_party",
        "internal_code",
    ] {
        assert_eq!(SourceClass::named(name).unwrap().name(), name);
    }
    assert_eq!(SourceClass::named("docs"), None);
    let mut set = SourceClassSet::default();
    assert!(!set.insert("vendor"));
    assert_eq!(set, SourceClassSet::default());
    assert!(set.insert("community"));
    assert!(set.insert("community"));
    assert!(set.contains(SourceClass::named("community").unwrap()));
    assert!(!set.contains(SourceClass::named("third_party").unwrap()));
}

#[test]
fn the_default_prior_demotes_repositories_community_and_third_party_only() {
    let SourcePrior::Soft { weight, classes } = SourcePrior::default() else {
        panic!("the default prior is official-first");
    };
    assert!((weight - 0.6).abs() < f32::EPSILON);
    let demoted = ["official_code", "community", "third_party", "internal_code"];
    for name in demoted {
        assert!(
            classes.contains(SourceClass::named(name).unwrap()),
            "{name}"
        );
    }
    for name in ["official_docs", "official_kb"] {
        assert!(
            !classes.contains(SourceClass::named(name).unwrap()),
            "{name}"
        );
    }
    assert!(SourcePrior::default().is_valid());
}

#[test]
fn the_prior_penalizes_only_its_classes_and_is_bounded() {
    let mut classes = SourceClassSet::default();
    assert!(classes.insert("community"));
    let prior = SourcePrior::Soft {
        weight: 0.25,
        classes,
    };
    assert_eq!(prior.multiplier(), Some(0.75));
    assert!(prior.penalizes(SourceClass::named("community").unwrap()));
    assert!(!prior.penalizes(SourceClass::named("official_docs").unwrap()));
    assert!(prior.is_active());
    assert!(!SourcePrior::Off.is_active());
    assert!(!SourcePrior::Off.penalizes(SourceClass::named("community").unwrap()));
    assert_eq!(SourcePrior::Off.multiplier(), None);
    assert!(
        !SourcePrior::Soft {
            weight: 0.5,
            classes: SourceClassSet::default()
        }
        .is_active()
    );
    for weight in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
        assert!(
            !SourcePrior::Soft { weight, classes }.is_valid(),
            "{weight}"
        );
    }
    for weight in [0.0, 1.0] {
        assert!(SourcePrior::Soft { weight, classes }.is_valid(), "{weight}");
    }
}

fn ranked(ids: &[&str]) -> Vec<Ranked> {
    ids.iter()
        .map(|id| Ranked {
            candidate: candidate(id, 1.0, id),
            score: Some(1.0),
        })
        .collect()
}

fn order(ranked: &[Ranked]) -> Vec<&str> {
    ranked
        .iter()
        .map(|item| item.candidate.fused.chunk_id.as_str())
        .collect()
}

#[test]
fn demotion_multiplies_reciprocal_ranks_and_keeps_order_without_penalties() {
    let mut items = ranked(&["0", "1", "2", "3", "4"]);
    rank_policy::demote(&mut items, &BTreeMap::new());
    assert_eq!(order(&items), ["0", "1", "2", "3", "4"]);
    rank_policy::demote(&mut items, &BTreeMap::from([("0".to_owned(), 0.5)]));
    assert_eq!(order(&items), ["1", "0", "2", "3", "4"]);
    let mut items = ranked(&["0", "1", "2", "3", "4"]);
    rank_policy::demote(
        &mut items,
        &BTreeMap::from([("0".to_owned(), 0.25), ("1".to_owned(), 0.5)]),
    );
    assert_eq!(order(&items), ["2", "3", "1", "0", "4"]);
}
