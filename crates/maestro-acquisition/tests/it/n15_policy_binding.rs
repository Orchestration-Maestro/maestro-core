//! Genuine admitted handles exercise each core policy binding independently.
use super::{n15_support as fixture, support};
use maestro_acquisition::extraction::{
    detect::DetectionEvidence,
    registry::{
        CheckedRegistry, LocalRegistry, ProfileSelection, RegistryUnavailable, checked_resolve,
    },
};
use maestro_acquisition::{Principal, ProfileRegistry, Ref};
use serde_json::json;

/// A substitute replays a genuine local handle resolved for an earlier platform.
#[derive(Debug)]
struct EarlierPlatform<'a> {
    local: LocalRegistry<'a>,
    platform: &'a str,
}
impl ProfileRegistry for EarlierPlatform<'_> {
    fn resolve(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<CheckedRegistry, RegistryUnavailable> {
        self.local.resolve(
            reference,
            &Principal {
                id: principal.id,
                platform: self.platform,
                scopes: principal.scopes,
            },
        )
    }
    fn select(
        &self,
        _checked: &CheckedRegistry,
        _evidence: &DetectionEvidence,
        _eligible: &[Ref],
    ) -> Result<ProfileSelection, RegistryUnavailable> {
        Err(RegistryUnavailable::Disabled)
    }
}

#[test]
fn n15_core_rejects_earlier_platform_under_the_same_principal() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let scopes = support::scopes();
    let mut principal = support::principal(&scopes);
    let adapter = EarlierPlatform {
        local: LocalRegistry::new(&catalog),
        platform: principal.platform,
    };
    assert!(adapter.resolve(&reference, &principal).is_ok());
    let policy = fixture::policy(&collection, &catalog);
    principal.platform = "later-host";
    assert_eq!(
        checked_resolve(&adapter, &reference, &principal, &policy).err(),
        Some(RegistryUnavailable::Access)
    );
}

#[test]
fn n15_core_rejects_wrong_collection_with_matching_scopes_and_visibility() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["collection_id"] = json!("another-collection");
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let scopes = support::scopes();
    let principal = support::principal(&scopes);
    let adapter = LocalRegistry::new(&catalog);
    assert!(adapter.resolve(&reference, &principal).is_ok());
    let policy = fixture::policy(&collection, &catalog);
    assert_eq!(
        checked_resolve(&adapter, &reference, &principal, &policy).err(),
        Some(RegistryUnavailable::Access)
    );
}

#[test]
fn n15_core_rejects_an_admitted_alternative_to_policy_profiles() {
    let (collection, mut catalog, reference) = fixture::registry_fixture();
    let policy = fixture::policy(&collection, &catalog);
    let mut registry = support::value(&catalog, "extraction");
    registry["id"] = json!("alternative-registry");
    let alternative = support::put(&mut catalog, "alternative-registry", &registry);
    let dependencies = catalog.0["extraction"].admission.references.clone();
    catalog
        .0
        .get_mut("alternative-registry")
        .unwrap()
        .admission
        .references = dependencies;
    assert_ne!(alternative, reference);
    let scopes = support::scopes();
    let principal = support::principal(&scopes);
    let adapter = LocalRegistry::new(&catalog);
    assert!(adapter.resolve(&alternative, &principal).is_ok());
    assert_eq!(
        checked_resolve(&adapter, &alternative, &principal, &policy).err(),
        Some(RegistryUnavailable::Access)
    );
}
