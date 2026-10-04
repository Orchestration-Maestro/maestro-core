//! Synthetic checked topology inputs and explicit review admission, tests only.

use super::super::{
    topology::compile,
    types::{Bindings, Edge, Node, NodeKind, ReviewEvidence, Workflow},
};
use crate::source::{Catalog, Maturity, Metadata, Resource, ResourceId, Value};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Architecture 03 §2.2's owner-relative location, not a type-first alias.
pub(super) const SOURCE: &str = "core/workflows/feature-delivery/workflow.md";
/// The otherwise-valid topology excerpt.
pub(super) const VALID: &str =
    include_str!("../../../../../../tests/fixtures/catalog/graphs/topology-valid.md");
/// Its independent missing-reviewer mutation.
pub(super) const INVALID: &str =
    include_str!("../../../../../../tests/fixtures/catalog/graphs/topology-invalid.md");

/// Test-only evidence explicitly admitted by exact ID, never read from source labels.
#[derive(Debug)]
pub(super) struct Reviewed(pub(super) BTreeSet<ResourceId>);
impl ReviewEvidence for Reviewed {
    fn reviewed(&self, id: &ResourceId) -> bool {
        self.0.contains(id)
    }
}

/// Only the reference/topology excerpt is decoded here; this is not a source parser.
#[derive(Deserialize)]
struct Excerpt {
    id: String,
    requires: Vec<String>,
    nodes: BTreeMap<String, ExcerptNode>,
    edges: Vec<String>,
}
/// The authored node fields in this reference excerpt.
#[derive(Deserialize)]
struct ExcerptNode {
    kind: String,
    agent: Option<String>,
    skill: Option<String>,
    #[serde(default)]
    independent_of: Vec<String>,
}

/// A syntactically valid exact ID for synthetic inputs.
pub(super) fn id(text: &str) -> ResourceId {
    ResourceId::parse(text).unwrap()
}

/// A checked catalog resource; labels and declarations are not evidence.
pub(super) fn resource(id: ResourceId, path: &str) -> Resource {
    Resource {
        id,
        path: path.to_owned(),
        files: vec![path.to_owned()],
        data: vec![],
        metadata: Metadata {
            maturity: Maturity::Reviewed,
            rows: vec!["architecture.L11".to_owned()],
            workflows: vec!["feature-delivery".to_owned()],
            requires: vec![],
            version: Some("1.2.0".to_owned()),
        },
        fields: BTreeMap::new(),
    }
}

/// Resolved bindings are compile inputs, not new authored fields or qualification.
pub(super) fn bindings(name: &str) -> Bindings {
    Bindings {
        session: Some(name.to_owned()),
        profile: Some(if name == "code" { "worker" } else { "reviewer" }.to_owned()),
        provider: Some("synthetic".to_owned()),
    }
}

/// Normalizes the fixed test excerpt and assembles its exact synthetic catalog/evidence.
pub(super) fn fixture(text: &str) -> (Workflow, Catalog, Reviewed) {
    let frontmatter = text.split("---").nth(1).unwrap();
    let excerpt: Excerpt = serde_yaml_ng::from_str(frontmatter).unwrap();
    let workflow = Workflow {
        source: SOURCE.to_owned(),
        id: id(&excerpt.id),
        requires: excerpt.requires.iter().map(|text| id(text)).collect(),
        start: "plan".to_owned(),
        nodes: excerpt
            .nodes
            .into_iter()
            .map(|(name, raw)| {
                let kind = match raw.kind.as_str() {
                    "agent" => NodeKind::Agent {
                        agent: raw.agent.unwrap(),
                        skill: raw.skill,
                    },
                    "step" => NodeKind::Step,
                    "join" => NodeKind::Join,
                    "gate" => NodeKind::Gate,
                    other => NodeKind::Unknown(other.to_owned()),
                };
                let node = Node {
                    kind,
                    terminal: name == "approve",
                    bindings: bindings(&name),
                    independent_of: raw.independent_of,
                };
                (name, node)
            })
            .collect(),
        edges: excerpt
            .edges
            .into_iter()
            .map(|text| {
                let (from, to) = text.split_once(" -> ").unwrap();
                Edge {
                    from: from.to_owned(),
                    to: to.to_owned(),
                    max_iterations: None,
                }
            })
            .collect(),
    };
    let mut root = resource(workflow.id.clone(), SOURCE);
    root.metadata.requires.clone_from(&workflow.requires);
    let mut area = resource(id("package:core"), "core/package.toml");
    area.fields.insert(
        "owners".to_owned(),
        Value::List(vec![Value::Text("synthetic-owner".to_owned())]),
    );
    let mut resources = vec![root, area];
    // The reviewer exists even in the omitted-requirement fixture: no basename fallback.
    for target in [
        "agent:core/planner",
        "agent:core/worker",
        "agent:core/reviewer",
        "skill:core/spec-compliance",
        "skill:core/security-review",
    ] {
        resources.push(resource(id(target), "core/synthetic"));
    }
    let evidence = Reviewed(
        resources
            .iter()
            .map(|resource| resource.id.clone())
            .collect(),
    );
    (
        workflow,
        Catalog {
            resources,
            ..Catalog::default()
        },
        evidence,
    )
}

/// Refuses at the real compiler, retaining the source and the offending key/reference.
pub(super) fn refuses(
    workflow: &Workflow,
    catalog: &Catalog,
    evidence: &Reviewed,
    offending: &str,
) {
    let result = compile(workflow, catalog, evidence);
    assert!(result.is_err(), "compiler accepted {offending:?}");
    let refusal = result.unwrap_err().to_string();
    assert!(refusal.contains(&workflow.source), "{refusal}");
    assert!(
        refusal.contains(offending),
        "missing {offending:?}: {refusal}"
    );
}
