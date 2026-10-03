//! YAML read node by node at injected limits: the exact node budget and
//! depth pass and one past refuses, and every alias replay counts.

use super::super::yaml::read;

#[test]
fn yaml_node_budget_boundary() {
    let yaml = "a: [b, c]\n";
    assert!(read(yaml, 32, 5).is_ok());
    assert_eq!(read(yaml, 32, 4).unwrap_err(), "more than 4 YAML nodes");
}

#[test]
fn yaml_alias_replays_count_against_the_budget() {
    let yaml = "a: &x [b, c]\nd: *x\n";
    assert!(read(yaml, 32, 9).is_ok());
    assert_eq!(read(yaml, 32, 8).unwrap_err(), "more than 8 YAML nodes");
}

#[test]
fn yaml_depth_is_refused_as_a_container_opens() {
    let yaml = "a: [[b], {c: d}]\n";
    assert!(read(yaml, 3, 100).is_ok());
    assert!(read(yaml, 2, 100).is_err(), "container depth must refuse");
    assert_eq!(read(yaml, 2, 100).unwrap_err(), "deeper than 2 levels");
}

#[test]
fn yaml_parser_errors_keep_their_own_message() {
    let error = read("a: [b\n", 32, 100).unwrap_err();
    assert!(error.starts_with("invalid frontmatter: "), "{error}");
}
