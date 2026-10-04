//! A kind's rules beyond its descriptor: a hook, which a descriptor selects
//! by name from the registry's fixed table and never supplies itself.

use super::types::{Known, Problems, Resource, ResourceId};
use std::{collections::BTreeMap, fmt};

/// A kind's rules beyond its descriptor; every method defaults to none.
pub(super) trait KindRules: fmt::Debug + Sync {
    /// Notes the problems within `resource`, whose Markdown body is `body`,
    /// with `known` as what the checker knows beyond the catalog.
    fn check_resource(
        &self,
        _resource: &Resource,
        _body: Option<&str>,
        _known: Known<'_>,
        _problems: &mut Problems,
    ) {
    }

    /// Exact inert payload paths owned by this decoded resource.
    fn assets(&self, _resource: &Resource) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    /// The resources `resource` depends on beyond its `requires`.
    fn edges(&self, _resource: &Resource) -> Vec<ResourceId> {
        Vec::new()
    }

    /// Notes the problems between `resource` and the other resources of the
    /// catalog, `catalog`.
    fn check_catalog(
        &self,
        _resource: &Resource,
        _catalog: &BTreeMap<ResourceId, &Resource>,
        _problems: &mut Problems,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        limits::Limits,
        source::{
            builtin,
            tests::support::{MemoryTree, check_under},
        },
    };

    #[test]
    fn default_kind_rules_add_no_assets_or_dependencies() {
        let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION);
        assert!(
            catalog.is_ok(),
            "default rules must preserve a valid catalog: {catalog:?}"
        );
        let catalog = catalog.unwrap();
        let resource = catalog
            .resources
            .iter()
            .find(|resource| resource.id.kind == "instructions")
            .unwrap();
        let registry = builtin().unwrap();
        let rules = registry.kind("instructions").unwrap().rules.unwrap();
        assert_eq!(rules.assets(resource), Ok(vec![]));
        assert!(rules.edges(resource).is_empty());
    }
}
