//! Synthetic scoped snapshots shared with the owning N30 conformance fixtures.
#![expect(clippy::indexing_slicing, reason = "authored synthetic inventory keys")]
use super::{n15_support, support};
use maestro_acquisition::adaptation::change::selection_key;
use maestro_acquisition::{
    Principal, Ref,
    adaptation::{
        artifacts::{
            ChunkSchema, CleanupRules, CleanupSchema, DedupKey, DedupKeys, DedupSchema,
            S1ChunkStrategy,
        },
        change::EffectiveConfiguration,
        snapshot::{ProcessingSnapshot, SnapshotReader, SnapshotSchema},
        storage,
    },
    extraction::{
        model::{Processing, Profile, ProfileDefinition},
        outcome::ProfileSelection,
    },
    policy::{decisions::Decisions, resource::Resource, schema::SourcePolicy},
    validate,
};
use maestro_kernel::{acquisition::Handle, artifact::Digest, gateway::Limits, store::Database};
use maestro_knowledge::collection::Declaration;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;

pub(super) const TAG: &str = "workspace/default/collection/garden";
/// Retain exact typed bytes without granting external qualification.
pub(super) fn retain(db: &Database, value: &impl Serialize, links: &[Handle]) -> Ref {
    storage::retain(
        db,
        &[TAG.into()],
        &serde_json::to_vec(value).unwrap(),
        links,
    )
    .unwrap()
}
/// Preserve the same shared resource metadata with a new closed schema.
pub(super) fn resource<S>(schema: S, id: &str, policy: &SourcePolicy) -> Resource<S> {
    Resource {
        schema,
        id: id.into(),
        version: 1.try_into().unwrap(),
        collection_id: policy.resource.collection_id.clone(),
        visibility: policy.resource.visibility,
        scope_tags: policy.resource.scope_tags.clone(),
        owner_ref: policy.resource.owner_ref.clone(),
    }
}
/// Independently scoped definitions and per-source defaults, without real qualification.
pub(super) fn processing_fixture(
    db: &Database,
    tags: &[String],
) -> (Declaration, support::Catalog, ProcessingSnapshot) {
    let (mut collection, mut catalog, _) = n15_support::registry_fixture();
    for id in ["addresses", "decisions", "http", "extraction", "policy"] {
        let mut value = support::value(&catalog, id);
        value["scope_tags"] = json!(tags);
        if id != "extraction" {
            support::bind(&mut value, &catalog);
        }
        let members = catalog.0[id].admission.references.clone();
        support::put(&mut catalog, id, &value);
        catalog.0.get_mut(id).unwrap().admission.references = members;
    }
    support::rebind(&mut collection, &mut catalog);
    let policy: SourcePolicy = serde_json::from_value(support::value(&catalog, "policy")).unwrap();
    let (model, tokenizer) = model(&mut catalog);
    let (defaults, approved_cleanup, approved_chunks, approved_dedup, qualified_chunk_tokens) =
        definitions(db, &mut catalog, &policy, &model, &tokenizer);
    let mut registry = support::value(&catalog, "extraction");
    for (index, profile) in registry["profiles"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        profile["processing"] = json!(defaults[usize::from(index == 2)]);
        n15_support::seal(profile, &mut catalog);
    }
    registry["unknown_profile"] = json!(catalog.0["safe"].reference);
    let registry_ref = n15_support::update(&mut collection, &mut catalog, &registry);
    let mut policy = support::value(&catalog, "policy");
    policy["profiles"] = json!(registry_ref);
    policy["adaptation"]["automatic_classes"] = json!([
        "selected_profiles",
        "cleanup",
        "s1_chunk_strategy",
        "dedup_keys",
        "new_knowledge_exclusions"
    ]);
    policy["sources"][0]["selected_profiles"] = json!([
        catalog.0["markdown"].reference,
        catalog.0["novel"].reference
    ]);
    let mut second = policy["sources"][0].clone();
    second["id"] = json!("second");
    for selector in second["selectors"].as_array_mut().unwrap() {
        selector["source_id"] = json!("second");
    }
    second["selected_profiles"] = json!([catalog.0["novel"].reference]);
    policy["sources"].as_array_mut().unwrap().push(second);
    support::put(&mut catalog, "policy", &policy);
    support::rebind(&mut collection, &mut catalog);
    let policy: SourcePolicy = serde_json::from_value(support::value(&catalog, "policy")).unwrap();
    let profiles: Vec<Profile> = serde_json::from_value(registry["profiles"].clone()).unwrap();
    let profiles: BTreeMap<_, _> = profiles
        .into_iter()
        .map(|profile| (profile.definition.id.clone(), profile))
        .collect();
    let (sources, mut protected_resources) = selections(db, &profiles, model, tokenizer);
    let declaration: Declaration = serde_json::from_value(collection.clone()).unwrap();
    let scopes = db.visible("synthetic-reader").unwrap();
    let checked = validate(&catalog, &declaration, &support::principal(&scopes)).unwrap();
    for reference in checked.references() {
        protected_resources.insert(reference.id.clone(), reference.clone());
    }
    let mut decisions = BTreeMap::new();
    for reference in &policy.registries {
        let registry: Decisions = serde_json::from_slice(&catalog.0[&reference.id].bytes).unwrap();
        for entry in registry.entries {
            decisions.insert(entry.id.clone(), (reference.clone(), entry));
        }
    }
    let snapshot = ProcessingSnapshot {
        resource: resource(SnapshotSchema::V1, "initial", &policy),
        effective: EffectiveConfiguration {
            baseline: catalog.0["policy"].reference.clone(),
            policy,
            sources,
            profiles,
            decisions,
            selected: (None, None, None),
            approved_cleanup,
            approved_chunks,
            approved_dedup,
            qualified_chunk_tokens,
            qualified_model_limit: 4096,
            protected_resources,
        },
    };
    (
        serde_json::from_value(collection).unwrap(),
        catalog,
        snapshot,
    )
}
/// Revalidate and retain through the actual shared reader.
pub(super) fn retain_snapshot(
    db: &Database,
    collection: &Declaration,
    catalog: &support::Catalog,
    snapshot: &ProcessingSnapshot,
) -> Ref {
    let scopes = db.visible("synthetic-reader").unwrap();
    let principal = support::principal(&scopes);
    reader(db, collection, catalog, &principal)
        .retain(snapshot)
        .unwrap()
}
pub(super) fn reader<'a>(
    db: &'a Database,
    collection: &'a Declaration,
    catalog: &'a support::Catalog,
    principal: &'a Principal<'a>,
) -> SnapshotReader<'a> {
    SnapshotReader {
        source: catalog,
        receipts: db,
        collection,
        principal,
    }
}

/// Existing v2 card, bound to synthetic tokenizer qualification bytes.
fn model(catalog: &mut support::Catalog) -> (Ref, Ref) {
    let tokenizer = support::put(
        catalog,
        "tokenizer-qualification",
        &json!({"synthetic":"tokenizer"}),
    );
    let card = include_str!("../fixtures/adaptation/model-v2.json")
        .trim()
        .replace(&"c".repeat(64), tokenizer.digest.as_str());
    let model = support::put(catalog, "model", &json!({"synthetic":"model"}));
    let value = catalog.0.get_mut(&model.id).unwrap();
    value.bytes = card.into_bytes();
    value.reference.digest = Digest::of(&value.bytes);
    value.admission.digest = value.reference.digest.clone();
    let model = value.reference.clone();
    (model, tokenizer)
}

/// Whole-set inventories, not another processing configuration.
type Inventories = (
    Vec<Processing>,
    BTreeMap<String, Ref>,
    BTreeMap<String, Ref>,
    BTreeMap<String, Ref>,
    BTreeMap<String, u64>,
);
/// Retain two independently qualified synthetic defaults.
fn definitions(
    db: &Database,
    catalog: &mut support::Catalog,
    policy: &SourcePolicy,
    model: &Ref,
    tokenizer: &Ref,
) -> Inventories {
    let qualification = support::put(
        catalog,
        "processing-qualification",
        &json!({"synthetic":"processing"}),
    );
    let mut defaults = Vec::new();
    let mut approved_cleanup = BTreeMap::new();
    let mut approved_chunks = BTreeMap::new();
    let mut approved_dedup = BTreeMap::new();
    let mut qualified_chunk_tokens = BTreeMap::new();
    for id in ["first", "second"] {
        let cleanup = retain(
            db,
            &CleanupRules {
                resource: resource(CleanupSchema::V1, id, policy),
                rules: vec![],
                qualification: qualification.clone(),
            },
            &[],
        );
        let chunk = retain(
            db,
            &S1ChunkStrategy {
                resource: resource(ChunkSchema::V1, id, policy),
                chunker_version: "mapped-structural-chunks/2".into(),
                preparation_profile: "canonical-context-parts/v1".into(),
                target_tokens: 500.try_into().unwrap(),
                hard_max_tokens: 700.try_into().unwrap(),
                model: model.clone(),
                tokenizer_qualification: tokenizer.clone(),
                model_limits: Limits {
                    context_tokens: 4096.try_into().unwrap(),
                    output_tokens: None,
                },
                qualification: qualification.clone(),
            },
            &[],
        );
        let dedup = retain(
            db,
            &DedupKeys {
                resource: resource(DedupSchema::V1, id, policy),
                exact: vec![DedupKey::OriginalDigest, DedupKey::CanonicalDigest],
                prepared: vec![
                    DedupKey::PreparedInputDigest,
                    DedupKey::EmbeddingProfileDigest,
                ],
                qualification: qualification.clone(),
            },
            &[],
        );
        catalog
            .0
            .get_mut(&qualification.id)
            .unwrap()
            .admission
            .references
            .extend([cleanup.clone(), chunk.clone(), dedup.clone()]);
        approved_cleanup.insert(cleanup.id.clone(), cleanup.clone());
        approved_chunks.insert(chunk.id.clone(), chunk.clone());
        approved_dedup.insert(dedup.id.clone(), dedup.clone());
        qualified_chunk_tokens.insert(chunk.id.clone(), 700);
        defaults.push(Processing {
            cleanup,
            chunk,
            dedup,
        });
    }
    (
        defaults,
        approved_cleanup,
        approved_chunks,
        approved_dedup,
        qualified_chunk_tokens,
    )
}

/// Source triples and their stored selection pins.
type Selections = (
    BTreeMap<String, (Ref, ProfileDefinition, Processing)>,
    BTreeMap<String, Ref>,
);
/// Pin the exact source selection and evidence for each synthetic source.
fn selections(
    db: &Database,
    profiles: &BTreeMap<String, Profile>,
    model: Ref,
    tokenizer: Ref,
) -> Selections {
    let mut protected_resources = BTreeMap::from([
        ("s1_embedding_model".into(), model),
        ("s1_tokenizer_qualification".into(), tokenizer),
    ]);
    let mut sources = BTreeMap::new();
    for (id, profile_id) in [("notes", "markdown"), ("second", "novel")] {
        let profile = &profiles[profile_id];
        let evidence =
            storage::retain(db, &[TAG.into()], b"synthetic N57 scoped canary", &[]).unwrap();
        let selection = retain(
            db,
            &ProfileSelection::Selected {
                profile: profile.reference(),
                evidence: vec![evidence.clone()],
            },
            &[evidence.id.parse().unwrap()],
        );
        protected_resources.insert(selection_key(id), selection);
        sources.insert(
            id.into(),
            (
                profile.reference(),
                profile.definition.clone(),
                profile.definition.processing.clone(),
            ),
        );
    }
    (sources, protected_resources)
}
