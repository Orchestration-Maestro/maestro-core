//! Policy resolution refuses each independent registry and owner violation.
use super::support;
use maestro_acquisition::{PolicySource, Refusal};
use maestro_knowledge::collection::Declaration;
use serde_json::json;

#[test]
fn policy_resolution_registry_duplicate_and_missing_evidence() {
    let scopes = support::scopes();
    for duplicate in [false, true] {
        let (mut collection, mut catalog) = support::fixture();
        let mut registry = support::value(&catalog, "decisions");
        if duplicate {
            let entry = registry["entries"][0].clone();
            registry["entries"].as_array_mut().unwrap().push(entry);
        } else {
            registry["entries"][0]["evidence"] = json!([]);
        }
        support::put(&mut catalog, "decisions", &registry);
        support::rebind(&mut collection, &mut catalog);
        let declaration = collection.to_string().parse::<Declaration>().unwrap();
        assert_eq!(
            catalog
                .resolve(&declaration, &support::principal(&scopes))
                .err(),
            Some(Refusal::Invalid),
            "duplicate {duplicate}"
        );
    }
}

#[test]
fn policy_resolution_scope_tags_and_principal_are_independent() {
    let scopes = support::scopes();
    for empty_tags in [false, true] {
        let (mut collection, mut catalog) = support::fixture();
        let mut policy = support::value(&catalog, "policy");
        let mut principal = support::principal(&scopes);
        if empty_tags {
            policy["scope_tags"] = json!([]);
        } else {
            principal.id = "";
        }
        support::put(&mut catalog, "policy", &policy);
        support::bind(&mut collection, &catalog);
        let declaration = collection.to_string().parse::<Declaration>().unwrap();
        assert_eq!(
            catalog.resolve(&declaration, &principal).err(),
            Some(Refusal::Access),
            "empty tags {empty_tags}"
        );
    }
}

#[test]
fn policy_resolution_collection_and_visibility_are_independent() {
    let scopes = support::scopes();
    for (field, value) in [("collection_id", "other"), ("visibility", "private")] {
        let (mut collection, mut catalog) = support::fixture();
        let mut policy = support::value(&catalog, "policy");
        policy[field] = json!(value);
        support::put(&mut catalog, "policy", &policy);
        support::bind(&mut collection, &catalog);
        let declaration = collection.to_string().parse::<Declaration>().unwrap();
        assert_eq!(
            catalog
                .resolve(&declaration, &support::principal(&scopes))
                .err(),
            Some(Refusal::Access),
            "{field}"
        );
    }
}
