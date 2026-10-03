//! Unmasked immutable-resource and policy-binding neighbours.
use super::{n15_support as fixture, support};
use maestro_acquisition::extraction::{
    detect::DetectionEvidence,
    registry::{
        CheckedRegistry, LocalRegistry, ProfileSelection, RegistryUnavailable, checked_resolve,
        checked_select,
    },
};
use maestro_acquisition::{Principal, ProfileRegistry, Ref};
use maestro_kernel::{artifact::Digest, scope::ScopeSet, store::Database};
use maestro_knowledge::strict_json::MAX_BYTES;
use maestro_test_scratch::scratch_directory;
use serde_json::json;
use std::fs;

#[test]
fn n15_missing_registry_has_no_default() {
    let (_, mut catalog, reference) = fixture::registry_fixture();
    catalog.0.remove("extraction");
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Missing)
    );
}

#[test]
fn n15_invalid_reference_id_refuses_before_read() {
    let (_, catalog, reference) = fixture::registry_fixture();
    let reference = Ref {
        id: "../bad".into(),
        digest: reference.digest,
    };
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_conflicting_closure_digests_refuse() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["owner_ref"]["digest"] = json!(Digest::of(b"different-owner"));
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let owner = catalog.0["owner"].reference.clone();
    catalog
        .0
        .get_mut("extraction")
        .unwrap()
        .admission
        .references
        .push(owner);
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_resource_closure_count_refuses() {
    let (_, mut catalog, reference) = fixture::registry_fixture();
    let mut refs = Vec::new();
    for index in 0..1000 {
        refs.push(support::put(
            &mut catalog,
            &format!("extra-{index}"),
            &json!({"synthetic":index}),
        ));
    }
    catalog
        .0
        .get_mut("extraction")
        .unwrap()
        .admission
        .references = refs;
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_oversized_pinned_artifact_refuses_before_use() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let artifact = catalog.0.get_mut("plugin").unwrap();
    artifact.bytes = vec![0; MAX_BYTES + 1];
    let digest = Digest::of(&artifact.bytes);
    artifact.reference.digest = digest.clone();
    artifact.admission.digest = digest;
    let mut registry = support::value(&catalog, "extraction");
    support::bind(&mut registry, &catalog);
    for profile in registry["profiles"].as_array_mut().unwrap() {
        fixture::seal(profile, &mut catalog);
    }
    support::bind(&mut registry, &catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

/// Returns an otherwise genuine core handle resolved for another caller.
#[derive(Debug)]
struct WrongPrincipal<'a>(LocalRegistry<'a>);
impl ProfileRegistry for WrongPrincipal<'_> {
    fn resolve(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<CheckedRegistry, RegistryUnavailable> {
        self.0.resolve(
            reference,
            &Principal {
                id: "another-reader",
                platform: principal.platform,
                scopes: principal.scopes,
            },
        )
    }
    fn select(
        &self,
        checked: &CheckedRegistry,
        evidence: &DetectionEvidence,
        eligible: &[Ref],
    ) -> Result<ProfileSelection, RegistryUnavailable> {
        self.0.select(checked, evidence, eligible)
    }
}
#[test]
fn n15_core_rejects_wrong_principal_handle_and_ineligible_profile() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let policy = fixture::policy(&collection, &catalog);
    let scopes = support::scopes();
    let principal = support::principal(&scopes);
    assert_eq!(
        checked_resolve(
            &WrongPrincipal(LocalRegistry::new(&catalog)),
            &reference,
            &principal,
            &policy
        )
        .err(),
        Some(RegistryUnavailable::Access)
    );
    let adapter = LocalRegistry::new(&catalog);
    let checked = checked_resolve(&adapter, &reference, &principal, &policy).unwrap();
    let sample =
        super::n15_route_content_through_one_extensible_profile_registry::evidence(b"opaque");
    // Safe retention is registry-owned, but cannot be an eligible automatic choice.
    assert_eq!(
        checked_select(
            &adapter,
            &checked,
            &sample,
            &[catalog.0["safe"].reference.clone()]
        ),
        Err(RegistryUnavailable::Corrupt)
    );
}

/// A substitute reuses old grants under the same principal/platform identity.
#[derive(Debug)]
struct StaleScopes<'a> {
    local: LocalRegistry<'a>,
    scopes: &'a ScopeSet,
}
impl ProfileRegistry for StaleScopes<'_> {
    fn resolve(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<CheckedRegistry, RegistryUnavailable> {
        self.local.resolve(
            reference,
            &Principal {
                id: principal.id,
                platform: principal.platform,
                scopes: self.scopes,
            },
        )
    }
    fn select(
        &self,
        checked: &CheckedRegistry,
        evidence: &DetectionEvidence,
        eligible: &[Ref],
    ) -> Result<ProfileSelection, RegistryUnavailable> {
        self.local.select(checked, evidence, eligible)
    }
}
#[test]
fn n15_core_rechecks_current_scopes_on_substitute_handle() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let policy = fixture::policy(&collection, &catalog);
    let prior_scopes = support::scopes();
    let directory = scratch_directory().unwrap();
    let database = Database::open_in(&directory).unwrap();
    let current_scopes = database.visible("synthetic-reader").unwrap();
    let adapter = StaleScopes {
        local: LocalRegistry::new(&catalog),
        scopes: &prior_scopes,
    };
    assert_eq!(
        checked_resolve(
            &adapter,
            &reference,
            &support::principal(&current_scopes),
            &policy
        )
        .err(),
        Some(RegistryUnavailable::Access)
    );
    drop(database);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn n15_each_qualification_resource_requires_external_admission() {
    for id in ["gold", "threshold", "output", "plugin"] {
        let (_, mut catalog, reference) = fixture::registry_fixture();
        fixture::unqualify(&mut catalog, id);
        assert_eq!(
            LocalRegistry::new(&catalog)
                .resolve(&reference, &support::principal(&support::scopes()))
                .err(),
            Some(RegistryUnavailable::Unqualified),
            "{id}"
        );
    }
}

#[test]
fn n15_pinned_plugin_substitution_refuses() {
    let (_, mut catalog, reference) = fixture::registry_fixture();
    catalog.0.get_mut("plugin").unwrap().bytes.push(0);
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}
