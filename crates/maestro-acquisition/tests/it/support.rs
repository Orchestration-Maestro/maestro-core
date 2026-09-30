//! Complete independently authored synthetic policy fixtures.
#![expect(
    clippy::indexing_slicing,
    reason = "fixture IDs are authored here; absent IDs must fail the test"
)]
use maestro_acquisition::{
    Admission, AdmissionStatus, CheckedPolicy, ImmutableResource, PolicySource, Principal, Ref,
    Refusal, ResourceSource, validate,
};
use maestro_kernel::{
    artifact::Digest,
    scope::{Right, ScopeSet},
    store::Database,
};
use maestro_knowledge::collection::Declaration;
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{collections::BTreeMap, env, fs};

/// A synthetic immutable catalog; it never starts a session or transport.
#[derive(Debug, Clone)]
pub(super) struct Catalog(pub BTreeMap<String, ImmutableResource>);

impl ResourceSource for Catalog {
    fn read(
        &self,
        reference: &Ref,
        _principal: &Principal<'_>,
    ) -> Result<ImmutableResource, Refusal> {
        self.0.get(&reference.id).cloned().ok_or(Refusal::Missing)
    }
}

/// Public synthetic context. Empty grants cannot open private policies.
pub(super) fn principal(scopes: &ScopeSet) -> Principal<'_> {
    Principal {
        id: "synthetic-reader",
        platform: env::consts::OS,
        scopes,
    }
}

/// An empty visible scope set, obtained from the real kernel.
pub(super) fn scopes() -> ScopeSet {
    let scratch = scratch_directory().unwrap();
    let database = Database::open_in(&scratch).unwrap();
    database
        .grant(
            "synthetic-reader",
            &"workspace/default/collection/garden".parse().unwrap(),
            Right::Read,
            "synthetic-owner",
        )
        .unwrap();
    let scopes = database.visible("synthetic-reader").unwrap();
    drop(database);
    fs::remove_dir_all(scratch).unwrap();
    scopes
}

/// Replace every exact reference with the fixture's corresponding digest.
pub(super) fn bind(value: &mut Value, catalog: &Catalog) {
    match value {
        Value::Object(fields) => {
            if fields.len() == 2 && fields.contains_key("digest") {
                let id = fields["id"].as_str().unwrap().to_owned();
                fields.insert("digest".into(), json!(catalog.0[&id].reference.digest));
            } else {
                for item in fields.values_mut() {
                    bind(item, catalog);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                bind(item, catalog);
            }
        }
        _ => {}
    }
}

/// Store bytes with separate trusted admission evidence.
pub(super) fn put(catalog: &mut Catalog, id: &str, value: &Value) -> Ref {
    let bytes = serde_json::to_vec(value).unwrap();
    let reference = Ref {
        id: id.into(),
        digest: Digest::of(&bytes),
    };
    let admission = Admission {
        digest: reference.digest.clone(),
        platform: env::consts::OS.into(),
        capabilities: vec![
            "fetch".into(),
            "http".into(),
            "browser_render".into(),
            "browser_request".into(),
        ],
        status: AdmissionStatus::Reviewed,
        references: vec![],
    };
    catalog.0.insert(
        id.into(),
        ImmutableResource {
            reference: reference.clone(),
            bytes,
            admission,
        },
    );
    reference
}

/// Build the full dependency closure without any private data.
pub(super) fn fixture() -> (Value, Catalog) {
    let mut catalog = Catalog(BTreeMap::new());
    for id in [
        "owner",
        "evidence",
        "qualification",
        "addresses",
        "matrix",
        "thresholds",
        "baseline",
        "retention",
        "adapter",
        "markdown",
    ] {
        put(&mut catalog, id, &json!({"synthetic": id}));
    }
    let markdown = catalog.0["markdown"].reference.clone();
    put(
        &mut catalog,
        "extraction",
        &json!({"synthetic":"extraction"}),
    );
    catalog
        .0
        .get_mut("extraction")
        .unwrap()
        .admission
        .references
        .push(markdown);
    for (id, text) in [
        ("decisions", include_str!("../fixtures/decisions.json")),
        ("http", include_str!("../fixtures/http.json")),
        ("policy", include_str!("../fixtures/policy.json")),
    ] {
        let mut value: Value = serde_json::from_str(text).unwrap();
        bind(&mut value, &catalog);
        put(&mut catalog, id, &value);
    }
    let mut collection: Value =
        serde_json::from_str(include_str!("../fixtures/collection.json")).unwrap();
    bind(&mut collection, &catalog);
    (collection, catalog)
}

/// Rebind the policy after changing a resource.
pub(super) fn rebind(collection: &mut Value, catalog: &mut Catalog) {
    let mut policy: Value = serde_json::from_slice(&catalog.0["policy"].bytes).unwrap();
    bind(&mut policy, catalog);
    put(catalog, "policy", &policy);
    bind(collection, catalog);
}

/// Read an editable resource fixture.
pub(super) fn value(catalog: &Catalog, id: &str) -> Value {
    serde_json::from_slice(&catalog.0[id].bytes).unwrap()
}

impl PolicySource for Catalog {
    fn resolve(
        &self,
        collection: &Declaration,
        principal: &Principal<'_>,
    ) -> Result<CheckedPolicy, Refusal> {
        validate(self, collection, principal)
    }
}

/// Exercise parser ceilings with values that remain otherwise valid JSON objects.
pub(super) fn parser_boundaries() {
    use maestro_knowledge::strict_json;
    let parse = |text: &str| strict_json::parse::<Value>(text.as_bytes());
    for depth in [32, 33] {
        let text = format!("{}0{}", "{\"a\":".repeat(depth), "}".repeat(depth));
        assert_eq!(parse(&text).is_ok(), depth == 32);
    }
    for items in [10_000, 10_001] {
        let text = format!("{{\"a\":[{}]}}", vec!["0"; items].join(","));
        assert_eq!(parse(&text).is_ok(), items == 10_000);
    }
    for items in [9_999, 10_000] {
        let text = format!(
            "{{\"a\":[{}],\"b\":[{}]}}",
            vec!["0"; items].join(","),
            vec!["0"; 9_999].join(",")
        );
        assert_eq!(parse(&text).is_ok(), items == 9_999);
    }
    let exact = format!("{{}}{}", " ".repeat(strict_json::MAX_BYTES - 2));
    assert!(parse(&exact).is_ok());
    assert!(parse(&(exact + " ")).is_err());
    assert!(parse("{\"a\":1,\"a\":2}").is_err());
}
