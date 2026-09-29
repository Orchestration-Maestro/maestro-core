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
