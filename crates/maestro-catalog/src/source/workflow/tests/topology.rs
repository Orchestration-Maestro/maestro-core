//! Each topology guard has a passing neighbour and an assertion-based refusal.

use super::super::{
    topology::compile,
    types::{Edge, NodeKind},
};
use super::support::{INVALID, SOURCE, VALID, fixture, id, refuses, resource};
use crate::source::Maturity;

#[test]
fn owner_first_example_references_accept() {
    let (workflow, catalog, evidence) = fixture(VALID);
    let closure = compile(&workflow, &catalog, &evidence).unwrap();
    let mut expected = workflow.requires.clone();
    expected.push(workflow.id.clone());
    expected.sort();
    assert_eq!(closure, expected);
    assert!(
        catalog
            .resources
            .iter()
            .all(|member| member.metadata.maturity == Maturity::Reviewed)
    );
    assert_eq!(workflow.source, SOURCE);
}

#[test]
fn legacy_example_references_refuse() {
    let (workflow, catalog, evidence) = fixture(VALID);
    let mut legacy = workflow.clone();
    legacy.source = "workflows/core/feature-delivery/workflow.md".to_owned();
    refuses(&legacy, &catalog, &evidence, &workflow.id.to_string());
    for (node, reference) in [
        ("code", "worker"),
        ("code", "core/agents/worker.agent.md"),
        ("spec", "spec-compliance"),
        ("spec", "core/skills/spec-compliance/SKILL.md"),
    ] {
        let mut legacy = workflow.clone();
        legacy.nodes.get_mut(node).unwrap().kind = NodeKind::Agent {
            agent: if node == "code" {
                reference
            } else {
                "agent:core/reviewer"
            }
            .to_owned(),
            skill: (node == "spec").then(|| reference.to_owned()),
        };
        refuses(&legacy, &catalog, &evidence, reference);
    }
    for target in &workflow.requires {
        let mut legacy = workflow.clone();
        legacy.requires.retain(|required| required != target);
        let mut catalog = catalog.clone();
        catalog
            .resources
            .first_mut()
            .unwrap()
            .metadata
            .requires
            .clone_from(&legacy.requires);
        refuses(&legacy, &catalog, &evidence, &target.to_string());
    }
}

#[test]
fn unreachable_node_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    let mut orphan = workflow.nodes.get("test").unwrap().clone();
    orphan.terminal = true;
    workflow.nodes.insert("orphan".to_owned(), orphan);
    refuses(&workflow, &catalog, &evidence, "orphan");
}

#[test]
fn path_without_terminal_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.edges.retain(|edge| edge.from != "security");
    refuses(&workflow, &catalog, &evidence, "security");
}

#[test]
fn bounded_repair_loop_accepts_unbounded_and_zero_refuse() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.edges.push(Edge {
        from: "test".to_owned(),
        to: "code".to_owned(),
        max_iterations: Some(3),
    });
    assert!(compile(&workflow, &catalog, &evidence).is_ok());
    for bound in [None, Some(0)] {
        workflow.edges.last_mut().unwrap().max_iterations = bound;
        refuses(&workflow, &catalog, &evidence, "cycle");
    }
}

#[test]
fn self_loop_needs_a_positive_bound() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.edges.push(Edge {
        from: "test".to_owned(),
        to: "test".to_owned(),
        max_iterations: Some(1),
    });
    assert!(compile(&workflow, &catalog, &evidence).is_ok());
    for bound in [None, Some(0)] {
        workflow.edges.last_mut().unwrap().max_iterations = bound;
        refuses(&workflow, &catalog, &evidence, "cycle");
    }
}

#[test]
fn every_cycle_needs_a_bound_not_merely_every_component() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.edges.push(Edge {
        from: "test".to_owned(),
        to: "code".to_owned(),
        max_iterations: Some(3),
    });
    workflow.edges.push(Edge {
        from: "reviewed".to_owned(),
        to: "code".to_owned(),
        max_iterations: None,
    });
    refuses(&workflow, &catalog, &evidence, "cycle");
    workflow.edges.last_mut().unwrap().max_iterations = Some(2);
    assert!(compile(&workflow, &catalog, &evidence).is_ok());
}

#[test]
fn bounded_map_accepts_unbounded_and_zero_refuse() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    for bound in [Some(1), None, Some(0)] {
        workflow.nodes.get_mut("test").unwrap().kind = NodeKind::Map { max_items: bound };
        if bound == Some(1) {
            assert!(compile(&workflow, &catalog, &evidence).is_ok());
        } else {
            refuses(&workflow, &catalog, &evidence, "test");
        }
    }
}

#[test]
fn bounded_subgraph_accepts_unbounded_zero_missing_and_undeclared_refuse() {
    let (mut workflow, mut catalog, mut evidence) = fixture(VALID);
    let target = id("workflow:core/child");
    workflow.requires.push(target.clone());
    catalog
        .resources
        .first_mut()
        .unwrap()
        .metadata
        .requires
        .clone_from(&workflow.requires);
    catalog
        .resources
        .push(resource(target.clone(), "core/workflows/child/workflow.md"));
    evidence.0.insert(target.clone());
    for bound in [Some(1), None, Some(0)] {
        workflow.nodes.get_mut("test").unwrap().kind = NodeKind::Subgraph {
            workflow: target.to_string(),
            max_depth: bound,
        };
        if bound == Some(1) {
            assert!(compile(&workflow, &catalog, &evidence).is_ok());
        } else {
            refuses(&workflow, &catalog, &evidence, "test");
        }
    }
    workflow.nodes.get_mut("test").unwrap().kind = NodeKind::Subgraph {
        workflow: target.to_string(),
        max_depth: Some(1),
    };
    evidence.0.remove(&target);
    refuses(&workflow, &catalog, &evidence, &target.to_string());
    evidence.0.insert(target.clone());
    catalog.resources.pop();
    refuses(&workflow, &catalog, &evidence, &target.to_string());
    catalog
        .resources
        .push(resource(target.clone(), "core/workflows/child/workflow.md"));
    workflow.requires.retain(|required| *required != target);
    catalog
        .resources
        .first_mut()
        .unwrap()
        .metadata
        .requires
        .clone_from(&workflow.requires);
    refuses(&workflow, &catalog, &evidence, &target.to_string());
}

#[test]
fn independent_reviewer_different_provider_accepts() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    let reviewer = &mut workflow.nodes.get_mut("spec").unwrap().bindings;
    reviewer.profile = Some("worker".to_owned());
    reviewer.provider = Some("other".to_owned());
    assert!(compile(&workflow, &catalog, &evidence).is_ok());
}

#[test]
fn reviewer_same_session_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.nodes.get_mut("spec").unwrap().bindings.session = Some("code".to_owned());
    refuses(&workflow, &catalog, &evidence, "independent_of");
}

#[test]
fn reviewer_same_profile_and_provider_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.nodes.get_mut("spec").unwrap().bindings.profile = Some("worker".to_owned());
    refuses(&workflow, &catalog, &evidence, "independent_of");
}

#[test]
fn reviewer_different_profile_shared_session_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    let reviewer = &mut workflow.nodes.get_mut("spec").unwrap().bindings;
    reviewer.profile = Some("different".to_owned());
    reviewer.session = Some("code".to_owned());
    refuses(&workflow, &catalog, &evidence, "independent_of");
}

#[test]
fn reviewer_missing_binding_on_either_side_refuses() {
    let (workflow, catalog, evidence) = fixture(VALID);
    for node in ["spec", "code"] {
        for field in ["session", "profile", "provider"] {
            for missing in [None, Some(String::new())] {
                let mut workflow = workflow.clone();
                let bindings = &mut workflow.nodes.get_mut(node).unwrap().bindings;
                match field {
                    "session" => bindings.session = missing,
                    "profile" => bindings.profile = missing,
                    _ => bindings.provider = missing,
                }
                refuses(&workflow, &catalog, &evidence, "independent_of");
            }
        }
    }
}

#[test]
fn reviewer_missing_named_node_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.nodes.get_mut("spec").unwrap().independent_of = vec!["absent".to_owned()];
    refuses(&workflow, &catalog, &evidence, "absent");
}

#[test]
fn unknown_construct_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.nodes.get_mut("test").unwrap().kind = NodeKind::Unknown("plugin".to_owned());
    refuses(&workflow, &catalog, &evidence, "plugin");
}

#[test]
fn missing_start_and_edge_endpoints_refuse() {
    let (workflow, catalog, evidence) = fixture(VALID);
    let mut invalid = workflow.clone();
    invalid.start = "absent-start".to_owned();
    refuses(&invalid, &catalog, &evidence, "absent-start");
    for from in [true, false] {
        let mut invalid = workflow.clone();
        let edge = invalid.edges.first_mut().unwrap();
        if from {
            edge.from = "absent-from".to_owned();
        } else {
            edge.to = "absent-to".to_owned();
        }
        refuses(
            &invalid,
            &catalog,
            &evidence,
            if from { "absent-from" } else { "absent-to" },
        );
    }
}

#[test]
fn supported_router_topology_accepts() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.nodes.get_mut("test").unwrap().kind = NodeKind::Router;
    assert!(compile(&workflow, &catalog, &evidence).is_ok());
}

#[test]
fn mandatory_reviewer_omitted_from_closure_refuses() {
    let (workflow, catalog, evidence) = fixture(INVALID);
    refuses(&workflow, &catalog, &evidence, "agent:core/reviewer");
}

#[test]
fn missing_reference_refuses() {
    let (workflow, mut catalog, evidence) = fixture(VALID);
    catalog
        .resources
        .retain(|resource| resource.id != id("agent:core/reviewer"));
    refuses(&workflow, &catalog, &evidence, "agent:core/reviewer");
}

#[test]
fn source_labels_never_supply_review_evidence() {
    let (workflow, catalog, mut evidence) = fixture(VALID);
    evidence.0.remove(&id("agent:core/reviewer"));
    refuses(&workflow, &catalog, &evidence, "agent:core/reviewer");
}

#[test]
fn ineligible_closure_members_refuse() {
    let (workflow, catalog, evidence) = fixture(VALID);
    for maturity in [
        Maturity::Placeholder,
        Maturity::Authored,
        Maturity::Retired,
        Maturity::Qualified,
    ] {
        let mut catalog = catalog.clone();
        catalog
            .resources
            .iter_mut()
            .find(|resource| resource.id == id("agent:core/reviewer"))
            .unwrap()
            .metadata
            .maturity = maturity;
        refuses(&workflow, &catalog, &evidence, "agent:core/reviewer");
    }
}
