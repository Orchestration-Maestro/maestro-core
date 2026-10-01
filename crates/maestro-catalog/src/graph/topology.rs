//! Static rules 1, 2, 3, 6 and 9. Positive loop bounds are removed before
//! checking for cycles: one bounded edge per component is not sufficient.

use super::types::{Bindings, NodeKind, ReviewEvidence, Workflow};
use crate::source::{
    Catalog, Diagnostic, Maturity, Problems, Refusal, Resource, ResourceId, closure::closure,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    slice::from_ref,
};

/// One architecture 03 §2.3 rule, kept in the sole covered-rule table.
struct Rule {
    /// The architecture rule number, retained in every refusal.
    number: u8,
    /// The static check implementing only that rule.
    check: fn(&mut Check<'_>),
}

/// The complete C22a scope; C22b adds the other seven rules.
const RULES: [Rule; 5] = [
    Rule {
        number: 1,
        check: references,
    },
    Rule {
        number: 2,
        check: reachability,
    },
    Rule {
        number: 3,
        check: cycles,
    },
    Rule {
        number: 6,
        check: independence,
    },
    Rule {
        number: 9,
        check: bounds,
    },
];

/// One effect-free check over admitted catalog data and resolved graph inputs.
struct Check<'a> {
    /// Normalized topology and original reference spellings.
    workflow: &'a Workflow,
    /// Checked resources, including area-derived ownership.
    catalog: &'a Catalog,
    /// Completed review admission, not editable declarations.
    evidence: &'a dyn ReviewEvidence,
    /// Exact resource lookup, with no basename or source-path index.
    resources: BTreeMap<&'a ResourceId, &'a Resource>,
    /// The visited exact forward closure, including the root.
    closure: BTreeSet<ResourceId>,
    /// Problems for the currently checked rule.
    problems: Problems,
}

/// Compiles only topology and its exact reviewed closure. No source admission,
/// qualification, route eligibility, scheduler or executable artifact is created.
pub(crate) fn compile(
    workflow: &Workflow,
    catalog: &Catalog,
    evidence: &dyn ReviewEvidence,
) -> Result<Vec<ResourceId>, Refusal> {
    let mut check = Check {
        workflow,
        catalog,
        evidence,
        resources: catalog
            .resources
            .iter()
            .map(|resource| (&resource.id, resource))
            .collect(),
        closure: BTreeSet::new(),
        problems: vec![],
    };
    let mut diagnostics = Vec::new();
    for rule in RULES {
        (rule.check)(&mut check);
        diagnostics.extend(check.problems.drain(..).map(|(key, message)| {
            Diagnostic::new(
                &workflow.source,
                key,
                format!("rule {}: {message}", rule.number),
            )
        }));
    }
    if diagnostics.is_empty() {
        Ok(check.closure.into_iter().collect())
    } else {
        diagnostics.sort();
        diagnostics.dedup();
        Err(Refusal { diagnostics })
    }
}

impl Check<'_> {
    /// Records one precise rule refusal.
    fn refuse(&mut self, key: impl Into<String>, message: impl Into<String>) {
        self.problems.push((key.into(), message.into()));
    }

    /// Checks a node reference's exact kind, declaration and catalog resolution.
    fn reference(&mut self, text: &str, kind: &str, key: &str) {
        let Some(id) = ResourceId::parse(text) else {
            self.refuse(key, format!("{text:?} is not a typed qualified {kind} ID"));
            return;
        };
        if id.kind != kind {
            self.refuse(key, format!("{text} must name a {kind}"));
        } else if !self.workflow.requires.contains(&id) {
            self.refuse(key, format!("{text} is not declared in requires"));
        }
    }
}

/// Rule 1: exact declared references and evidence for every closure member.
fn references(check: &mut Check<'_>) {
    let workflow = check.workflow;
    let Some(root) = check.resources.get(&workflow.id).copied() else {
        check.refuse("id", format!("{} does not exist", workflow.id));
        return;
    };
    let canonical = ResourceId::parse(&workflow.id.to_string());
    if workflow.id.kind != "workflow" || canonical.as_ref() != Some(&workflow.id) {
        check.refuse(
            "id",
            format!("{} is not a qualified workflow ID", workflow.id),
        );
    }
    let expected = check.catalog.ownership(root).map(|ownership| {
        let area = ownership
            .descriptor
            .path
            .strip_suffix("package.toml")
            .unwrap_or_default();
        format!("{area}workflows/{}/workflow.md", workflow.id.name)
    });
    if expected.as_deref() != Some(workflow.source.as_str()) || root.path != workflow.source {
        check.refuse(
            "id",
            format!(
                "{} requires its owner-relative source, expected {expected:?}",
                workflow.id
            ),
        );
    }
    if workflow.requires != root.metadata.requires {
        check.refuse(
            "requires",
            format!(
                "{} requirements differ from the checked resource",
                workflow.id
            ),
        );
    }
    let closure = closure(
        &check.resources,
        from_ref(&workflow.id),
        |_| Vec::new(),
        &mut check.problems,
    );
    for id in &closure {
        let Some(resource) = check.resources.get(id).copied() else {
            continue;
        };
        if ResourceId::parse(&id.to_string()).as_ref() != Some(id) {
            check.refuse("requires", format!("{id} is not a typed qualified ID"));
        }
        if resource.metadata.maturity != Maturity::Reviewed || !check.evidence.reviewed(id) {
            check.refuse(
                "requires",
                format!("{id} needs reviewed maturity and admitted review evidence"),
            );
        }
    }
    check.closure = closure;
    for (name, node) in &workflow.nodes {
        match &node.kind {
            NodeKind::Agent { agent, skill } => {
                check.reference(agent, "agent", &format!("nodes.{name}.agent"));
                if let Some(skill) = skill {
                    check.reference(skill, "skill", &format!("nodes.{name}.skill"));
                }
            }
            NodeKind::Subgraph { workflow, .. } => {
                check.reference(workflow, "workflow", &format!("nodes.{name}.workflow"));
            }
            NodeKind::Unknown(kind) => check.refuse(
                format!("nodes.{name}.kind"),
                format!("unsupported construct {kind:?}"),
            ),
            NodeKind::Step
            | NodeKind::Gate
            | NodeKind::Router
            | NodeKind::Map { .. }
            | NodeKind::Join => {}
        }
    }
}

/// Walks graph-local edges in either direction without recursion.
fn reached(workflow: &Workflow, roots: Vec<String>, reverse: bool) -> BTreeSet<String> {
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &workflow.edges {
        let (from, to) = if reverse {
            (&edge.to, &edge.from)
        } else {
            (&edge.from, &edge.to)
        };
        adjacency.entry(from).or_default().push(to);
    }
    let mut pending = roots;
    let mut seen = BTreeSet::new();
    while let Some(node) = pending.pop() {
        if seen.insert(node.clone()) {
            pending.extend(
                adjacency
                    .get(node.as_str())
                    .into_iter()
                    .flatten()
                    .map(|target| (*target).to_owned()),
            );
        }
    }
    seen
}

/// Rule 2: explicit start/endpoints, all nodes reachable and terminal-reaching.
fn reachability(check: &mut Check<'_>) {
    let workflow = check.workflow;
    if !workflow.nodes.contains_key(&workflow.start) {
        check.refuse("start", format!("node {:?} does not exist", workflow.start));
    }
    for edge in &workflow.edges {
        for endpoint in [&edge.from, &edge.to] {
            if !workflow.nodes.contains_key(endpoint) {
                check.refuse("edges", format!("node {endpoint:?} does not exist"));
            }
        }
    }
    let forward = reached(workflow, vec![workflow.start.clone()], false);
    let terminals = workflow
        .nodes
        .iter()
        .filter(|(_, node)| node.terminal)
        .map(|(name, _)| name.clone())
        .collect();
    let backward = reached(workflow, terminals, true);
    for name in workflow.nodes.keys() {
        if !forward.contains(name) {
            check.refuse(format!("nodes.{name}"), "unreachable from start");
        }
        if !backward.contains(name) {
            check.refuse(format!("nodes.{name}"), "path cannot reach a terminal");
        }
    }
}

/// Rule 3: Kahn's acyclic check on edges without a positive iteration bound.
fn cycles(check: &mut Check<'_>) {
    let workflow = check.workflow;
    let mut indegree: BTreeMap<&str, usize> = workflow
        .nodes
        .keys()
        .map(|name| (name.as_str(), 0))
        .collect();
    let mut targets: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &workflow.edges {
        if edge.max_iterations.is_some_and(|bound| bound > 0) {
            continue;
        }
        if workflow.nodes.contains_key(&edge.from)
            && let Some(count) = indegree.get_mut(edge.to.as_str())
        {
            *count += 1;
            targets.entry(&edge.from).or_default().push(&edge.to);
        }
    }
    let mut pending: Vec<&str> = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(name, _)| *name)
        .collect();
    let mut visited = 0;
    while let Some(node) = pending.pop() {
        visited += 1;
        for target in targets.get(node).into_iter().flatten() {
            let Some(count) = indegree.get_mut(target) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                pending.push(target);
            }
        }
    }
    if visited != workflow.nodes.len() {
        check.refuse("edges", "cycle without a positive max_iterations back-edge");
    }
}

/// Complete, nonempty resolved bindings; missing facts fail closed.
fn resolved(bindings: &Bindings) -> Option<(&str, &str, &str)> {
    let session = bindings
        .session
        .as_deref()
        .filter(|text| !text.trim().is_empty())?;
    let profile = bindings
        .profile
        .as_deref()
        .filter(|text| !text.trim().is_empty())?;
    let provider = bindings
        .provider
        .as_deref()
        .filter(|text| !text.trim().is_empty())?;
    Some((session, profile, provider))
}

/// Rule 6: distinct sessions AND a different profile OR provider.
fn independence(check: &mut Check<'_>) {
    for (name, node) in &check.workflow.nodes {
        for other in &node.independent_of {
            let key = format!("nodes.{name}.independent_of");
            let Some(target) = check.workflow.nodes.get(other) else {
                check.refuse(key, format!("node {other:?} does not exist"));
                continue;
            };
            let pair = resolved(&node.bindings).zip(resolved(&target.bindings));
            let Some((reviewer, author)) = pair else {
                check.refuse(
                    key,
                    format!(
                        "{name} and {other} need resolved session, profile and provider bindings"
                    ),
                );
                continue;
            };
            if reviewer.0 == author.0 || (reviewer.1 == author.1 && reviewer.2 == author.2) {
                check.refuse(
                    key,
                    format!(
                        "{name} cannot be independent of {other}: {}",
                        "needs a distinct session and different profile or provider"
                    ),
                );
            }
        }
    }
}

/// Rule 9: every map/depth ceiling is explicitly positive. Total nested
/// workflow depth and mutual recursion are C22b's source-compilation obligation.
fn bounds(check: &mut Check<'_>) {
    for (name, node) in &check.workflow.nodes {
        let limit = match &node.kind {
            NodeKind::Map { max_items } => Some(("max_items", *max_items)),
            NodeKind::Subgraph { max_depth, .. } => Some(("max_depth", *max_depth)),
            _ => None,
        };
        if let Some((field, limit)) = limit
            && limit.is_none_or(|limit| limit == 0)
        {
            check.refuse(
                format!("nodes.{name}.{field}"),
                "must declare a positive bound",
            );
        }
    }
}
