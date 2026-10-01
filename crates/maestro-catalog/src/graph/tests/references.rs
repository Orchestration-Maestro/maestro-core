//! Exact reference closure, lifecycle evidence and typed identity vectors.

use super::super::{topology::compile, types::NodeKind};
use super::support::{VALID, fixture, id, refuses, resource};
use crate::source::ResourceId;

#[test]
fn exact_transitive_closure_ignores_labels_and_unrelated_members() {
    let (workflow, mut catalog, mut evidence) = fixture(VALID);
    let target = id("skill:core/extra-check");
    catalog
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("agent:core/reviewer"))
        .unwrap()
        .metadata
        .requires
        .push(target.clone());
    catalog
        .resources
        .push(resource(target.clone(), "core/skills/extra-check/SKILL.md"));
    catalog.resources.push(resource(
        id("skill:core/unrelated"),
        "core/skills/unrelated/SKILL.md",
    ));
    evidence.0.insert(target.clone());
    let closure = compile(&workflow, &catalog, &evidence).unwrap();
    assert!(closure.contains(&target));
    assert!(!closure.contains(&id("skill:core/unrelated")));
    assert!(!closure.contains(&id("package:core")));
    catalog
        .resources
        .last_mut()
        .unwrap()
        .metadata
        .requires
        .push(target.clone());
    assert_eq!(compile(&workflow, &catalog, &evidence).unwrap(), closure);
    evidence.0.remove(&target);
    refuses(&workflow, &catalog, &evidence, &target.to_string());
}

#[test]
fn wrong_reference_kind_refuses() {
    let (mut workflow, catalog, evidence) = fixture(VALID);
    workflow.nodes.get_mut("code").unwrap().kind = NodeKind::Agent {
        agent: "skill:core/spec-compliance".to_owned(),
        skill: None,
    };
    refuses(&workflow, &catalog, &evidence, "skill:core/spec-compliance");
}

#[test]
fn typed_identity_serialization_golden() {
    assert_eq!(
        serde_json::to_vec(&id("workflow:core/feature-delivery")).unwrap(),
        br#"{"kind":"workflow","namespace":"core","name":"feature-delivery"}"#
    );
}

#[test]
fn workflow_identity_source_and_requirements_must_match_catalog() {
    let (workflow, catalog, evidence) = fixture(VALID);
    let mut absent = catalog.clone();
    absent
        .resources
        .retain(|resource| resource.id != workflow.id);
    refuses(&workflow, &absent, &evidence, &workflow.id.to_string());
    let mut absent_owner = catalog.clone();
    absent_owner
        .resources
        .retain(|resource| resource.id != id("package:core"));
    refuses(
        &workflow,
        &absent_owner,
        &evidence,
        &workflow.id.to_string(),
    );
    let mut wrong_path = catalog.clone();
    wrong_path.resources.first_mut().unwrap().path = "core/workflows/other/workflow.md".to_owned();
    refuses(&workflow, &wrong_path, &evidence, "owner-relative");
    let mut different = workflow.clone();
    different.requires.clear();
    refuses(&different, &catalog, &evidence, "requirements differ");
    let mut invalid = workflow.clone();
    invalid.id.kind = "agent".to_owned();
    let mut invalid_catalog = catalog.clone();
    invalid_catalog
        .resources
        .first_mut()
        .unwrap()
        .id
        .clone_from(&invalid.id);
    refuses(
        &invalid,
        &invalid_catalog,
        &evidence,
        "qualified workflow ID",
    );
    invalid.id.kind = "workflow".to_owned();
    invalid.id.namespace = None;
    invalid_catalog
        .resources
        .first_mut()
        .unwrap()
        .id
        .clone_from(&invalid.id);
    refuses(
        &invalid,
        &invalid_catalog,
        &evidence,
        "qualified workflow ID",
    );
    let mut legacy = workflow.clone();
    legacy.source = "workflows/core/feature-delivery/workflow.md".to_owned();
    let mut legacy_catalog = catalog.clone();
    legacy_catalog
        .resources
        .first_mut()
        .unwrap()
        .path
        .clone_from(&legacy.source);
    refuses(&legacy, &legacy_catalog, &evidence, "owner-relative");
}

#[test]
fn transitive_missing_and_malformed_requirements_refuse() {
    let (workflow, catalog, evidence) = fixture(VALID);
    let target = id("skill:core/absent");
    let mut missing = catalog.clone();
    missing
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("agent:core/reviewer"))
        .unwrap()
        .metadata
        .requires
        .push(target.clone());
    refuses(&workflow, &missing, &evidence, "does not exist");
    let target = ResourceId {
        kind: "skill".to_owned(),
        namespace: None,
        name: "alias".to_owned(),
    };
    let mut malformed = catalog;
    let mut evidence = evidence;
    malformed
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("agent:core/reviewer"))
        .unwrap()
        .metadata
        .requires
        .push(target.clone());
    malformed
        .resources
        .push(resource(target.clone(), "core/skills/alias/SKILL.md"));
    evidence.0.insert(target);
    refuses(&workflow, &malformed, &evidence, "not a typed qualified ID");
}

#[test]
fn closure_visits_shared_requirements_only_once() {
    use super::super::types::ReviewEvidence;
    use super::support::Reviewed;
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    /// A review source proves each exact identity is consulted at most once.
    struct Once {
        reviewed: Reviewed,
        seen: RefCell<BTreeSet<ResourceId>>,
    }
    impl ReviewEvidence for Once {
        fn reviewed(&self, id: &ResourceId) -> bool {
            assert!(
                self.seen.borrow_mut().insert(id.clone()),
                "reviewed twice: {id}"
            );
            self.reviewed.reviewed(id)
        }
    }
    let (workflow, mut catalog, evidence) = fixture(VALID);
    catalog
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("agent:core/reviewer"))
        .unwrap()
        .metadata
        .requires
        .push(id("skill:core/spec-compliance"));
    let once = Once {
        reviewed: evidence,
        seen: RefCell::new(BTreeSet::new()),
    };
    assert!(compile(&workflow, &catalog, &once).is_ok());
    assert_eq!(once.seen.borrow().len(), 6);
}
