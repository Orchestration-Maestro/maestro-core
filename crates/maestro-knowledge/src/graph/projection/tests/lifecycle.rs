//! Public lifecycle refusals paired with valid neighboring configurations.
use crate::graph::projection::EngineSettings;
#[cfg(feature = "engine")]
use crate::graph::{descriptors::tests_source::Authority, projection::ProjectionBuild};
use maestro_kernel::artifact::Digest;
#[cfg(feature = "engine")]
use std::time::SystemTime;

#[test]
fn graph_settings_accept_exact_bounds_and_refuse_adjacent_invalid_values() {
    let min = 16 * 1024 * 1024;
    let pool_max = 1024 * 1024 * 1024;
    let db_max = 1024 * 1024 * 1024 * 1024;
    let lock = Digest::of(b"frozen lock");
    for pool in [min, pool_max] {
        for size in [min, db_max] {
            for threads in [1, 64] {
                let config = EngineSettings::new(pool, size, threads, lock.clone()).unwrap();
                assert_eq!(config.frozen_lock(), &lock);
            }
        }
    }
    for (pool, size, threads) in [
        (min - 1, min, 1),
        (pool_max + 1, min, 1),
        (min, min - 1, 1),
        (min, db_max + 1, 1),
        (min, min + 1, 1),
        (min, min, 0),
        (min, min, 65),
    ] {
        assert!(EngineSettings::new(pool, size, threads, lock.clone()).is_err());
    }
}

#[cfg(feature = "engine")]
#[test]
fn lifecycle_public_producer_requires_existing_guards_and_allows_valid_neighbor() {
    use crate::graph::projection::{ProjectionEngine, ProjectionError, ProjectionFactory};
    use maestro_canonicalization::{ControlFile, OwnedRoot, SystemFileLock};
    use maestro_test_scratch::scratch_directory;
    use std::fs;
    let (authority, build, now) = authority_build();
    let path = scratch_directory().unwrap();
    let settings =
        EngineSettings::new(16 * 1024 * 1024, 64 * 1024 * 1024, 1, Digest::of(b"lock")).unwrap();
    let root = OwnedRoot::open(&path, false).unwrap();
    let factory =
        ProjectionFactory::new(&path, ProjectionEngine::Ladybug, settings, &SystemFileLock);
    let clock = || now;
    let disabled = ProjectionFactory::new(
        &path,
        ProjectionEngine::None,
        factory_settings(),
        &SystemFileLock,
    );
    for refusal in [
        disabled
            .reader(&authority.database, &authority.scopes, build.scope.clone())
            .map(|_| ()),
        disabled
            .producer(
                &authority.database,
                &authority.scopes,
                build.clone(),
                &clock,
            )
            .map(|_| ()),
    ] {
        assert!(matches!(refusal, Err(ProjectionError::Backend(message))
                if message.contains("graph.engine = none")));
    }
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    let error = factory
        .producer(
            &authority.database,
            &authority.scopes,
            build.clone(),
            &clock,
        )
        .unwrap_err();
    let remedy = "maestro setup --yes";
    assert!(matches!(error, ProjectionError::Backend(message) if message.contains(remedy)));
    let error = factory
        .reader(&authority.database, &authority.scopes, build.scope.clone())
        .unwrap_err();
    assert!(matches!(error, ProjectionError::Backend(message) if message.contains(remedy)));
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
    #[cfg(not(windows))]
    drop(
        factory
            .producer(&authority.database, &authority.scopes, build, &clock)
            .unwrap(),
    );
    #[cfg(windows)]
    assert!(
        factory
            .producer(&authority.database, &authority.scopes, build, &clock)
            .is_err()
    );
    drop(root);
    fs::remove_dir_all(path).unwrap();
}

#[cfg(feature = "engine")]
fn factory_settings() -> EngineSettings {
    EngineSettings::new(16 * 1024 * 1024, 64 * 1024 * 1024, 1, Digest::of(b"lock")).unwrap()
}

#[test]
fn lifecycle_cancellation_token_is_one_way_and_shared_between_clones() {
    use crate::graph::projection::ProjectionCancellation;
    let token = ProjectionCancellation::new();
    let other = token.clone();
    assert!(!token.is_cancelled());
    other.cancel();
    assert!(token.is_cancelled());
    token.cancel();
    assert!(other.is_cancelled());
}

#[cfg(feature = "engine")]
fn authority_build() -> (Authority, ProjectionBuild, SystemTime) {
    use crate::graph::{
        descriptors::tests_source::Authority,
        projection::{ProjectionBuild, ProjectionScope},
    };
    use maestro_kernel::{job::NewJob, scope::collection_path};
    use serde_json::json;
    use std::time::{Duration, SystemTime};
    let authority = Authority::new();
    let scope = ProjectionScope {
        collection_id: authority.pin.collection_id.clone(),
        generation_id: authority.pin.generation_id,
    };
    let set = authority
        .database
        .graph_attachment(&authority.scopes, scope.generation_id)
        .unwrap()
        .unwrap()
        .claim_set_id;
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let body = json!({"generation": scope.generation_id});
    let kernel_scope = collection_path(&scope.collection_id).parse().unwrap();
    let job = authority
        .database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.project",
                inputs: &body,
                scope: &kernel_scope,
                resource: None,
            },
            now,
        )
        .unwrap();
    let lease = authority
        .database
        .take_job(job.id, "producer", now, Duration::from_secs(60))
        .unwrap();
    let build = ProjectionBuild {
        scope,
        claim_set_id: set,
        lease,
    };
    (authority, build, now)
}

#[test]
fn lifecycle_expected_verification_reuses_the_encoder_and_refuses_invalid_input() {
    use super::contract;
    use crate::graph::projection::{BuildVerification, EdgeFamily, ProjectionError, content};
    use std::slice;
    let edge = contract::edge(7, EdgeFamily::KnowledgeClaim);
    let fact = contract::fact(&edge.scope);
    let expected =
        BuildVerification::expected(slice::from_ref(&edge), slice::from_ref(&fact)).unwrap();
    assert_eq!(
        expected.content_digest,
        content::digest(slice::from_ref(&edge), slice::from_ref(&fact)).unwrap()
    );
    assert_eq!(expected.fact_count, 1);
    assert_eq!(expected.family_counts[&EdgeFamily::KnowledgeClaim], 1);
    let mut invalid = edge.clone();
    invalid.relation.clear();
    assert!(matches!(
        BuildVerification::expected(&[invalid], &[]),
        Err(ProjectionError::Invalid(_))
    ));
    let mut invalid = fact;
    invalid.scope.generation_id += 1;
    assert!(matches!(
        BuildVerification::expected(&[edge], &[invalid]),
        Err(ProjectionError::Invalid(_))
    ));
    assert_eq!(BuildVerification::expected(&[], &[]).unwrap().fact_count, 0);
}

#[test]
fn lifecycle_expected_catalog_requires_registered_vocabulary_and_valid_neighbor() {
    use super::contract;
    use crate::graph::projection::{BuildVerification, CatalogRelationVocabulary, EdgeFamily};
    use std::slice;
    struct Vocabulary;
    impl CatalogRelationVocabulary for Vocabulary {
        fn accepts(&self, relation: &str) -> bool {
            relation == "depends_on"
        }
    }
    let mut edge = contract::edge(7, EdgeFamily::CatalogDependency);
    edge.relation = "depends_on".into();
    assert!(BuildVerification::expected(slice::from_ref(&edge), &[]).is_err());
    assert!(
        BuildVerification::expected_with_catalog_vocabulary(
            slice::from_ref(&edge),
            &[],
            &Vocabulary
        )
        .is_ok()
    );
    edge.relation = "unregistered".into();
    assert!(
        BuildVerification::expected_with_catalog_vocabulary(&[edge], &[], &Vocabulary).is_err()
    );
}

#[test]
fn lifecycle_expected_scope_requires_a_valid_collection_and_positive_generation() {
    use super::contract;
    use crate::graph::projection::{BuildVerification, EdgeFamily};
    use std::slice;
    let edge = contract::edge(7, EdgeFamily::KnowledgeClaim);
    for (collection, generation) in [("", 7), ("c", 0), ("c", -1), ("bad\0scope", 7)] {
        let mut invalid = edge.clone();
        invalid.scope.collection_id = collection.into();
        invalid.scope.generation_id = generation;
        assert!(BuildVerification::expected(&[invalid], &[]).is_err());
    }
    assert!(BuildVerification::expected(slice::from_ref(&edge), &[]).is_ok());
    let fact = contract::fact(&edge.scope);
    assert_eq!(
        BuildVerification::expected(&[], &[fact])
            .unwrap()
            .fact_count,
        1
    );
}
