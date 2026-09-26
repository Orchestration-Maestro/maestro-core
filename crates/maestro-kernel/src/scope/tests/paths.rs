//! Scope paths: a workspace, then a collection, then a source, each named by
//! the rule collection and source IDs follow, and what a scope covers.

use super::support::scope;
use crate::scope::{Scope, check_name};
use std::error;

#[test]
fn a_scope_is_a_workspace_then_a_collection_then_a_source() {
    for path in [
        "workspace/default",
        "workspace/default/collection/ctm",
        "workspace/default/collection/ctm/source/docs-core",
        "workspace/7/collection/9.0.22/source/a_b",
    ] {
        let parsed: Scope = path.parse().unwrap();
        assert_eq!(parsed.as_str(), path);
        assert_eq!(parsed.to_string(), path);
    }
}

#[test]
fn a_path_off_the_kinds_or_their_order_is_refused_naming_it() {
    for path in [
        "",
        "/",
        "workspace",
        "/workspace/default",
        "workspace/default/",
        "collection/ctm",
        "source/docs-core",
        "workspace/default/source/docs-core",
        "workspace/default/project/maestro",
        "workspace/default/collection/ctm/collection/ctm",
        "workspace/default/collection/ctm/source",
        "workspace/default/collection/ctm/source/docs-core/source/extra",
        "Workspace/default",
        "workspace/default/Collection/ctm",
    ] {
        let refusal = path.parse::<Scope>().unwrap_err();
        let message = refusal.to_string();
        assert!(
            message.starts_with(&format!("`{path}` is not a scope")),
            "{message}"
        );
        assert!(message.contains("`workspace/<name>`"), "{message}");
        assert!(error::Error::source(&refusal).is_none(), "{path}");
    }
}

#[test]
fn a_path_with_a_name_off_the_rule_is_refused_naming_both() {
    for (path, name) in [
        ("workspace/", ""),
        ("workspace//collection/ctm", ""),
        ("workspace/Default", "Default"),
        ("workspace/default/collection/CTM", "CTM"),
        ("workspace/default/collection/c t m", "c t m"),
        ("workspace/default/collection/ctm/source/-docs", "-docs"),
        ("workspace/default/collection/ctm\\source", "ctm\\source"),
    ] {
        let refusal = path.parse::<Scope>().unwrap_err();
        let message = refusal.to_string();
        assert!(
            message.starts_with(&format!("`{path}` is not a scope")),
            "{message}"
        );
        assert!(
            message.contains(&format!("`{name}` is not a scope name")),
            "{message}"
        );
        let cause = error::Error::source(&refusal).map(ToString::to_string);
        assert_eq!(
            cause,
            check_name(name).err().map(|invalid| invalid.to_string())
        );
    }
}

#[test]
fn a_name_is_lowercase_ascii_letters_digits_and_three_marks() {
    let longest = "a".repeat(64);
    for name in [
        "ctm",
        "docs-core",
        "default",
        "a",
        "7",
        "a.b_c-d",
        "9.0.22",
        longest.as_str(),
    ] {
        check_name(name).unwrap();
    }
    let too_long = "a".repeat(65);
    for name in [
        "",
        "-ctm",
        ".ctm",
        "_ctm",
        "..",
        "CTM",
        "Ctm",
        "docs core",
        "docs/core",
        "docs:core",
        "docs\\core",
        "\u{e9}t\u{e9}",
        "ctm\n",
        "\u{212a}elvin",
        too_long.as_str(),
    ] {
        let refusal = check_name(name).unwrap_err();
        let message = refusal.to_string();
        assert!(
            message.starts_with(&format!("`{name}` is not a scope name")),
            "{message}"
        );
        assert!(message.contains("1 to 64 characters"), "{message}");
    }
}

#[test]
fn a_scope_covers_itself_and_its_descendants_by_whole_segments() {
    let workspace = scope("workspace/default");
    let ct = scope("workspace/default/collection/ct");
    let ctm = scope("workspace/default/collection/ctm");
    let docs = scope("workspace/default/collection/ctm/source/docs-core");
    for (granted, below) in [
        (&workspace, &workspace),
        (&workspace, &ctm),
        (&workspace, &docs),
        (&ctm, &ctm),
        (&ctm, &docs),
        (&docs, &docs),
    ] {
        assert!(granted.covers(below), "{granted} covers {below}");
    }
    for (granted, other) in [
        (&ct, &ctm),
        (&ct, &docs),
        (&ctm, &workspace),
        (&docs, &ctm),
        (&ctm, &scope("workspace/default/collection/ctm-archive")),
        (&scope("workspace/def"), &ctm),
        (&scope("workspace/other"), &ctm),
        (
            &scope("workspace/default/collection/ctm/source/docs"),
            &docs,
        ),
    ] {
        assert!(!granted.covers(other), "{granted} does not cover {other}");
    }
}
