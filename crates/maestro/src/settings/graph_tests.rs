//! Typed native consumers use the admitted snapshot, not a second defaults producer.
use super::{GraphActivationError, GraphEngine, KnowledgeSettings, Session};
use maestro_catalog::{
    limits::Limits,
    settings::{AdmissionError, WorkspacePreferences},
};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::graph::projection::EngineSettings;
use maestro_settings::{Discovery, Layers, Registry};
use std::{collections::BTreeSet, path::Path};

/// Already admitted source with an opaque complete lock identity.
struct Source {
    registry: Registry,
    identity: String,
}
impl WorkspacePreferences for Source {
    fn registry(&self) -> Result<Registry, String> {
        Ok(self.registry.clone())
    }
    fn layers(&self, _: &Registry, _: &Limits) -> Result<Layers, String> {
        Ok(Layers::default())
    }
    fn frozen_lock(&self) -> Option<&str> {
        Some(&self.identity)
    }
}

#[test]
fn native_graph_settings_carry_all_resolved_values_and_the_complete_lock() {
    let lock = Digest::of(b"complete authoring lock including all non-resource sources");
    let source = Source {
        registry: Registry::built_in().unwrap(),
        identity: format!("sha256:{}", lock.as_str()),
    };
    let flags = [
        "graph.engine=ladybug",
        "graphdb.buffer_pool_size=33554432",
        "graphdb.max_db_size=134217728",
        "graphdb.max_num_threads=3",
    ]
    .map(str::to_owned);
    let session =
        Session::from_preferences(Path::new("config"), &source, Discovery::default(), &flags)
            .unwrap();
    assert_eq!(
        session.graph_settings().unwrap(),
        Some(EngineSettings::new(33_554_432, 134_217_728, 2, lock.clone()).unwrap())
    );
    let narrowed_flags = ["graph.engine=ladybug", "graphdb.max_num_threads=1"].map(str::to_owned);
    let narrowed = Session::from_preferences(
        Path::new("config"),
        &source,
        Discovery::default(),
        &narrowed_flags,
    )
    .unwrap();
    assert_eq!(
        narrowed.graph_settings().unwrap(),
        Some(EngineSettings::new(256 * 1024 * 1024, 16 * 1024 * 1024 * 1024, 1, lock).unwrap())
    );
}

#[test]
fn native_graph_settings_refuse_absent_or_invalid_lock_and_non_power_of_two_size() {
    let mut source = Source {
        registry: Registry::built_in().unwrap(),
        identity: "invalid".into(),
    };
    let read = |source: &Source, flags: &[String]| {
        Session::from_preferences(Path::new("config"), source, Discovery::default(), flags)
            .and_then(|session| session.graph_settings())
    };
    assert!(read(&source, &["graph.engine=ladybug".into()]).is_err());
    source.identity = format!("sha256:{}", Digest::of(b"lock").as_str());
    assert!(
        read(
            &source,
            &[
                "graph.engine=ladybug".into(),
                "graphdb.max_db_size=67108865".into()
            ]
        )
        .is_err()
    );
    let session = Session::from_preferences(
        Path::new("config"),
        &source,
        Discovery::default(),
        &["graph.engine=ladybug".into()],
    )
    .unwrap();
    let mut missing = session;
    missing.frozen_lock = None;
    assert!(
        missing
            .graph_settings()
            .unwrap_err()
            .to_string()
            .contains("authoring lock")
    );
    missing.flags.clear();
    assert_eq!(
        missing.graph_settings().unwrap(),
        None,
        "none needs no identity"
    );
}

#[test]
fn graph_session_uses_manifest_defaults_and_the_real_complete_lock_read_only() {
    use crate::cli::session;
    use maestro_catalog::{
        files::{self, FileInput, FilePlan},
        policy::workspace::{CheckedTrust, TrustBoundaries},
        settings::{SessionPreferences, WorkspaceTrust},
    };
    use maestro_test_scratch::scratch_directory;
    use std::{fs, path::PathBuf};
    struct Root(PathBuf);
    impl WorkspaceTrust for Root {
        fn containing_root(&self, path: &Path) -> Option<PathBuf> {
            path.starts_with(&self.0).then(|| self.0.clone())
        }
    }
    let scratch = scratch_directory().unwrap().canonicalize().unwrap();
    let root = scratch.join("project");
    fs::create_dir(&root).unwrap();
    let boundaries = TrustBoundaries::new(&scratch, &[]).unwrap();
    let adapter = Root(root.clone());
    let trust = CheckedTrust::new(&adapter, &boundaries);
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema": "maestro-authoring-lock/3",
        "defaults": concat!(
            "schema = 'maestro-preferences/1'\n[overrides]\n'graph.engine' = 'ladybug'\n",
            "'graphdb.buffer_pool_size' = 33554432\n'graphdb.max_db_size' = 134217728\n",
            "'graphdb.max_num_threads' = 3\n"
        ),
        "backend_types": ["ladybug"], "files": [],
        "sources": [{
            "path": "core/backends/graphdb/config.toml", "sha256": "synthetic-source-identity"
        }]
    }))
    .unwrap();
    let plan = FilePlan::preview(
        &root,
        [FileInput::new(
            ".maestro/authoring.lock.json",
            bytes.clone(),
        )],
        &trust,
    )
    .unwrap();
    files::apply(&root, &plan, &trust).unwrap();
    let config = scratch.join("user");
    let load = || {
        SessionPreferences::load(
            &config,
            Some(&root),
            Some(&scratch),
            &trust,
            &Limits::PRODUCTION,
        )
        .unwrap()
    };
    let health = session::health_at(&config, Some(&root), Some(&scratch), &[], &trust).unwrap();
    assert_eq!(
        fs::read(root.join(".maestro/authoring.lock.json")).unwrap(),
        bytes
    );
    if cfg!(feature = "engine") {
        let admitted = load()
            .admit_defaults(&trust, &session::compiled_backends(), &Limits::PRODUCTION)
            .unwrap();
        let runtime =
            Session::from_preferences(&config, &admitted, admitted.discovery.clone(), &[]).unwrap();
        let expected =
            Some(EngineSettings::new(33_554_432, 134_217_728, 3, Digest::of(&bytes)).unwrap());
        assert_eq!(runtime.graph_settings().unwrap(), expected);
        assert_eq!(health.graph_settings().unwrap(), expected);
    } else {
        assert!(matches!(
            load().admit_defaults(&trust, &session::compiled_backends(), &Limits::PRODUCTION),
            Err(AdmissionError::BackendNotCompiled { backend, .. }) if backend == "ladybug"
        ));
        assert_eq!(
            health.graph_activation_error.as_ref().unwrap(),
            &GraphActivationError::EngineMissing
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn every_registered_setting_is_read_by_a_consumer() {
    let source = Source {
        registry: Registry::built_in().unwrap(),
        identity: String::new(),
    };
    let branches: [&[&str]; 2] = [
        &[],
        &[
            "search.section_prior.weight=0.5",
            "search.rerank.context=bounded_section",
            "search.source_prior.weight=off",
        ],
    ];
    let mut read = BTreeSet::new();
    for flags in branches {
        let flags: Vec<_> = flags.iter().map(|flag| (*flag).to_owned()).collect();
        let session =
            Session::from_preferences(Path::new("config"), &source, Discovery::default(), &flags)
                .unwrap();
        read.extend(KnowledgeSettings::read(&session.resolved()).unwrap().1);
        read.extend(GraphEngine::read(&session).unwrap().1);
    }
    let catalog = [
        "updates",
        "model_profile",
        "reasoning_effort",
        "inference_writers",
        "workspace_writers",
        "delegation_depth",
        "tool_calls",
        "repair_attempts",
        "routing_candidates",
        "mcp_call_timeout",
        "cross_project_memory",
        "mcp_apps",
        "extensions",
        "schedules",
        "raw_prompt_logging",
        "raw_reasoning_logging",
        "provider_fallback",
        "evidence_validation",
        "result_validation",
        "discovered_executable_hooks",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    assert!(read.is_disjoint(&catalog));
    // Native graph tuning is checked separately under the engine feature;
    // workflow budgets belong to the deferred workflow consumer.
    let deferred = [
        "graphdb.buffer_pool_size",
        "graphdb.max_db_size",
        "graphdb.max_num_threads",
        "workflow.budgets.tokens",
        "workflow.budgets.wall_ms",
        "workflow.budgets.tool_calls",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    let covered = read
        .union(&catalog)
        .cloned()
        .chain(deferred)
        .collect::<BTreeSet<_>>();
    let registered = source
        .registry
        .descriptors()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        covered, registered,
        "each registered key has an actual reader or a named owner"
    );
}
