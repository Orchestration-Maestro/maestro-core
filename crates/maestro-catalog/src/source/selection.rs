//! Checked selection admission, separate from partial source validation.

use super::{
    Catalog, Diagnostic, Maturity, Problems, Refusal, Registry, Resource, ResourceId, Value,
    closure::closure,
};
use std::collections::{BTreeMap, BTreeSet};

impl Catalog {
    /// Resolves a checked root closure in ID order: common, core, all standards,
    /// canonical Maestro and the explicit selection, with transitive requirements.
    /// Explicit-plus-implicit roots are deduplicated, never selected twice.
    /// Partial source trees remain checkable; no install path calls this seam yet.
    /// Declaration/offline validation confers no GitHub approval or runtime grant.
    ///
    /// # Errors
    /// Refuses missing, duplicate, unreviewed, retired-area or ownership-inconsistent
    /// selections, including a member whose area descriptor is not selected.
    pub fn selection(
        &self,
        selected: &[ResourceId],
        registry: &Registry,
    ) -> Result<Vec<&Resource>, Refusal> {
        let mut diagnostics = Vec::new();
        let mut resources = BTreeMap::new();
        for resource in &self.resources {
            if resources.insert(&resource.id, resource).is_some() {
                diagnostics.push(Diagnostic::new(
                    &resource.path,
                    "id",
                    format!("duplicate ID {}", resource.id),
                ));
            }
        }
        let mut explicit = BTreeSet::new();
        for id in selected {
            if !explicit.insert(id) {
                diagnostics.push(Diagnostic::new(
                    "",
                    "selection",
                    format!("duplicate selection {id}"),
                ));
            }
        }
        let mut roots = selected.to_vec();
        roots.extend(["common", "core"].map(|name| ResourceId {
            kind: "package".to_owned(),
            namespace: None,
            name: name.to_owned(),
        }));
        roots.push(ResourceId {
            kind: "agent".to_owned(),
            namespace: Some("core".to_owned()),
            name: "maestro".to_owned(),
        });
        roots.extend(
            self.resources
                .iter()
                .filter(|resource| resource.id.kind == "standard")
                .map(|resource| resource.id.clone()),
        );
        let mut problems = Problems::new();
        let ids = closure(
            &resources,
            &roots,
            |resource| {
                registry
                    .kind(&resource.id.kind)
                    .and_then(|registration| registration.rules)
                    .map(|rules| rules.edges(resource))
                    .unwrap_or_default()
            },
            &mut problems,
        );
        diagnostics.extend(
            problems
                .into_iter()
                .map(|(key, message)| Diagnostic::new("", key, message)),
        );
        let mut members = Vec::new();
        for id in &ids {
            let Some(resource) = resources.get(id).copied() else {
                continue;
            };
            if ResourceId::parse(&id.to_string()).as_ref() != Some(id)
                || registry.kind(&id.kind).is_none()
            {
                diagnostics.push(Diagnostic::new(
                    &resource.path,
                    "id",
                    format!("{id} is not a registered typed qualified ID"),
                ));
            }
            if resource.metadata.maturity != Maturity::Reviewed {
                diagnostics.push(Diagnostic::new(
                    &resource.path,
                    "maturity",
                    format!("{id} needs reviewed maturity"),
                ));
            }
            if let Some(problem) = self.selection_ownership_problem(resource, &ids) {
                diagnostics.push(problem);
            }
            members.push(resource);
        }
        if diagnostics.is_empty() {
            Ok(members)
        } else {
            diagnostics.sort();
            diagnostics.dedup();
            Err(Refusal { diagnostics })
        }
    }

    /// Selection checks the owning area's status independently of member maturity.
    fn selection_ownership_problem(
        &self,
        resource: &Resource,
        selected: &BTreeSet<ResourceId>,
    ) -> Option<Diagnostic> {
        let Some(ownership) = self.ownership(resource) else {
            return Some(Diagnostic::new(
                &resource.path,
                "ownership",
                format!("{} does not match its area ownership record", resource.id),
            ));
        };
        if !selected.contains(&ownership.descriptor.id) {
            return Some(Diagnostic::new(
                &resource.path,
                "selection",
                format!(
                    "unselected area: {}; {} requires {} in the selection",
                    ownership.descriptor.id.name, resource.id, ownership.descriptor.id
                ),
            ));
        }
        if ownership
            .descriptor
            .fields
            .get("status")
            .and_then(Value::text)
            == Some("retired")
        {
            return Some(Diagnostic::new(
                &resource.path,
                "status",
                format!(
                    "{} belongs to retired area {}",
                    resource.id, ownership.descriptor.id
                ),
            ));
        }
        None
    }
}
