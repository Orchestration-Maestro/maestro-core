//! Shared/cyclic closures remain bounded by the existing logical resource cache.
use super::n15_route_content_through_one_extensible_profile_registry::evidence;
use super::{n15_support as fixture, support};
use maestro_acquisition::extraction::registry::{
    LocalRegistry, ProfileSelection, RegistryUnavailable,
};
use maestro_acquisition::{
    ImmutableResource, Principal, ProfileRegistry, Ref, Refusal, ResourceSource,
};
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap};

/// Count immutable reads by identity, without changing admission.
#[derive(Debug)]
struct Counted<'a> {
    catalog: &'a support::Catalog,
    reads: RefCell<BTreeMap<String, usize>>,
}
impl ResourceSource for Counted<'_> {
    fn read(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<ImmutableResource, Refusal> {
        *self
            .reads
            .borrow_mut()
            .entry(reference.id.clone())
            .or_default() += 1;
        self.catalog.read(reference, principal)
    }
}

#[test]
fn n15_diamond_and_cycle_qualification_read_each_dependency_once() {
    let (_, mut catalog, reference) = fixture::registry_fixture();
    let child = support::put(&mut catalog, "diamond-child", &json!({"synthetic":"child"}));
    let plugin = catalog.0["plugin"].reference.clone();
    for parent in ["plugin", "gold"] {
        catalog
            .0
            .get_mut(parent)
            .unwrap()
            .admission
            .references
            .push(child.clone());
    }
    catalog
        .0
        .get_mut("diamond-child")
        .unwrap()
        .admission
        .references
        .push(plugin);
    let counted = Counted {
        catalog: &catalog,
        reads: RefCell::new(BTreeMap::new()),
    };
    let adapter = LocalRegistry::new(&counted);
    let checked = adapter
        .resolve(&reference, &support::principal(&support::scopes()))
        .unwrap();
    assert!(matches!(
        adapter
            .select(
                &checked,
                &evidence(b"NEW format"),
                &[catalog.0["novel"].reference.clone()]
            )
            .unwrap(),
        ProfileSelection::Selected { .. }
    ));
    assert_eq!(counted.reads.borrow()["diamond-child"], 1);
    assert!(counted.reads.borrow().values().all(|reads| *reads == 1));
}

#[test]
fn n15_qualification_walk_has_a_1000_resource_boundary() {
    let (_, mut catalog, reference) = fixture::registry_fixture();
    let counted = Counted {
        catalog: &catalog,
        reads: RefCell::new(BTreeMap::new()),
    };
    LocalRegistry::new(&counted)
        .resolve(&reference, &support::principal(&support::scopes()))
        .unwrap();
    let baseline = counted.reads.borrow().len();
    for index in baseline..1000 {
        let child = support::put(
            &mut catalog,
            &format!("dependency-{index}"),
            &json!({"synthetic":index}),
        );
        catalog
            .0
            .get_mut("plugin")
            .unwrap()
            .admission
            .references
            .push(child);
    }
    let counted = Counted {
        catalog: &catalog,
        reads: RefCell::new(BTreeMap::new()),
    };
    let adapter = LocalRegistry::new(&counted);
    let checked = adapter
        .resolve(&reference, &support::principal(&support::scopes()))
        .unwrap();
    assert!(matches!(
        adapter
            .select(
                &checked,
                &evidence(b"NEW format"),
                &[catalog.0["novel"].reference.clone()]
            )
            .unwrap(),
        ProfileSelection::Selected { .. }
    ));
    assert_eq!(counted.reads.borrow().len(), 1000);
    let child = support::put(&mut catalog, "overflow", &json!({"synthetic":"overflow"}));
    catalog
        .0
        .get_mut("plugin")
        .unwrap()
        .admission
        .references
        .push(child);
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}
