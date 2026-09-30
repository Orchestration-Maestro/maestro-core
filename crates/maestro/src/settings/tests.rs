//! The session's settings: every default is today's behaviour, each
//! setting reaches the knowledge operations, and the CLI and the MCP server
//! resolve the same files the same way.

use super::{Compute, KnowledgeSettings, Session};
use crate::failure::Failure;
use maestro_kernel::evidence::RequestBudget;
use maestro_knowledge::{
    answer::{AnswerPrompt, AskBudget, DEFAULT_MODEL, Presentation, PromptVersion, Tone},
    prepare::ChunkProfile,
    search::{
        CandidateContext, IntentExpansion, IntentTrigger, SearchConfiguration, SectionClassSet,
        SectionPrior, SourceClass, SourceClassSet, SourcePrior, StageWindow,
        evidence::{CounterMode, EvidenceSettings, ExpansionMode},
    },
};
use maestro_settings::{
    LayerName, PROJECT_DIRECTORY, PROJECT_FILE, Registry, SettingKind, Source, USER_FILE,
};
use maestro_test_scratch::scratch_directory;
use std::{
    collections::BTreeSet,
    fs,
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
    time::Duration,
};

/// A scratch directory holding `config/` and `home/`, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// A new one.
    fn new() -> Self {
        let root = scratch_directory().unwrap().canonicalize().unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("home").join("work")).unwrap();
        Self(root)
    }

    /// The configuration directory.
    fn config(&self) -> PathBuf {
        self.0.join("config")
    }

    /// The home.
    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    /// Writes the user file with `body` after its schema.
    fn user(&self, body: &str) {
        fs::write(
            self.config().join(USER_FILE),
            format!("schema = \"maestro-preferences/1\"\n{body}"),
        )
        .unwrap();
    }

    /// Writes the home's project file with `body` after its schema.
    fn project(&self, body: &str) -> PathBuf {
        let folder = self.home().join(PROJECT_DIRECTORY);
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join(PROJECT_FILE);
        fs::write(&file, format!("schema = \"maestro-preferences/1\"\n{body}")).unwrap();
        file
    }

    /// The session started in the home's `work/`, with `flags`.
    fn session(&self, flags: &[&str]) -> Session {
        let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
        Session::at(
            &self.config(),
            Some(&self.home().join("work")),
            Some(&self.home()),
            &flags,
        )
        .unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// The knowledge settings of the defaults.
fn defaults() -> KnowledgeSettings {
    let scratch = Scratch::new();
    scratch.session(&[]).knowledge().unwrap()
}

#[test]
fn every_default_is_the_measured_default() {
    let today = KnowledgeSettings {
        search: SearchConfiguration::default(),
        evidence: EvidenceSettings::default(),
        search_budget: RequestBudget {
            evidence_bytes: RequestBudget::DEFAULT_SEARCH_EVIDENCE_BYTES,
            ..RequestBudget::default()
        },
        ask_budget: AskBudget::default(),
        model: DEFAULT_MODEL.to_owned(),
        prompt: AnswerPrompt::Presented {
            version: PromptVersion::default(),
            presentation: Presentation::default(),
        },
        compute: Compute::Gpu,
        chunk_profile: ChunkProfile::CompleteIdeas,
    };
    assert_eq!(defaults(), today);
    assert_eq!(KnowledgeSettings::default(), today);
}

#[test]
fn rerank_depth_cannot_exceed_the_configured_fusion_pool() {
    let scratch = Scratch::new();
    scratch.user("[search]\nfusion_pool = 20\n[search.rerank]\ndepth = 21\n");
    let error = scratch.session(&[]).knowledge().unwrap_err().to_string();
    assert!(error.contains("search.rerank.depth (21) must not exceed search.fusion_pool (20)"));
}

#[test]
fn the_descriptors_name_exactly_the_values_their_consumers_accept() {
    let registry = Registry::built_in().unwrap();
    let values = |key: &str| match &registry.get(key).unwrap().kind {
        SettingKind::Choice { values, .. } | SettingKind::ChoiceList { values } => {
            values.iter().map(ToString::to_string).collect::<Vec<_>>()
        }
        other => panic!("{key} is {other:?}"),
    };
    let profiles: Vec<String> = ChunkProfile::ALL
        .iter()
        .map(|profile| profile.chunker_version().to_owned())
        .collect();
    assert_eq!(values("chunking.profile"), profiles);
    for class in values("search.section_prior.classes") {
        assert!(SectionClassSet::default().insert(&class), "{class}");
    }
    for class in values("search.source_prior.classes") {
        assert_eq!(
            SourceClass::named(&class).map(SourceClass::name),
            Some(class.as_str())
        );
    }
    assert_eq!(
        values("search.intent.expansion"),
        [IntentExpansion::Off, IntentExpansion::Hyde].map(|expansion| serde_json::to_value(
            expansion
        )
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned())
    );
    assert_eq!(
        values("tone"),
        [Tone::Brief, Tone::Normal, Tone::Detailed].map(|tone| tone.name().to_owned())
    );
    assert_eq!(
        values("ask.prompt"),
        [
            PromptVersion::V1,
            PromptVersion::V2,
            PromptVersion::ProcedureFirst
        ]
        .map(|version| version.name().to_owned())
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "this full configuration assertion pins every mapped setting"
)]
fn each_setting_reaches_the_knowledge_operations() {
    let scratch = Scratch::new();
    scratch.user(
        "language = \"fr-CA\"\ntone = \"detailed\"\n\
         [search]\nk = 12\nevidence_bytes = 4000\ndeadline_ms = 9000\nrrf_k = 40\n\
         stage_window_ms = 1200\n\
         routes = { dense = false, lexical = true, identifier = false, \
         structured = false, limit = 32, identifier_limit = 8 }\n\
         weights = { dense = 0.5, lexical = 2, identifier = 3, \
         structured = 0 }\nidentifier = { noise_guard = true }\n\
         [search.rerank]\ndepth = 50\nblend = 0.25\ndemotion_cap = 4\n\
         context = \"bounded_section\"\n\
         context_max_bytes = 900\n\
         [search.section_prior]\nweight = 0.6\nclasses = [\"conversion\"]\n\
         [search.intent]\nexpansion = \"hyde\"\nmin_top_rerank = 0.5\ndeadline_ms = 2500\n\
         weight = 0.75\nrerank_additions = 4\n\
         [search.source_prior]\nweight = 0.3\nclasses = [\"community\"]\n\
         [evidence]\nexpansion = \"relevant_blocks\"\ncounter = \"utf8_answer_bound\"\n\
         [ask]\nmodel = \"qwen3-8b\"\nprompt = \"procedure_first\"\nk = 7\nevidence_bytes = 3000\n\
         search_deadline_ms = 8000\noutput_tokens = 512\nmin_rerank_score = -1.5\n\
         [chunking]\nprofile = \"mapped-structural-chunks/2\"\n",
    );
    let settings = scratch.session(&[]).knowledge().unwrap();
    let mut classes = SectionClassSet::default();
    assert!(classes.insert("conversion"));
    let mut sources = SourceClassSet::default();
    assert!(sources.insert("community"));
    assert_eq!(
        settings.search,
        SearchConfiguration {
            dense_enabled: false,
            lexical_enabled: true,
            identifier_enabled: false,
            structured_enabled: false,
            identifier_noise_guard: true,
            rrf_k: NonZeroU32::new(40).unwrap(),
            dense_weight: 0.5,
            lexical_weight: 2.0,
            identifier_weight: 3.0,
            structured_weight: 0.0,
            rerank_enabled: true,
            routes_limit: 32,
            identifier_limit: 8,
            fusion_pool: 120,
            rerank_depth: NonZeroUsize::new(50).unwrap(),
            min_rerank_score: Some(-1.5),
            rerank_blend: Some(0.25),
            rerank_demotion_cap: Some(4),
            candidate_context: CandidateContext::BoundedSection { max_bytes: 900 },
            section_prior: SectionPrior::Soft {
                weight: 0.6,
                classes,
            },
            stage_window: StageWindow::Fixed(Duration::from_millis(1200)),
            intent_expansion: IntentExpansion::Hyde,
            intent_trigger: IntentTrigger::LowConfidence {
                min_top_rerank: 0.5
            },
            intent_deadline_ms: 2500,
            intent_weight: 0.75,
            intent_rerank_additions: 4,
            source_prior: SourcePrior::Soft {
                weight: 0.3,
                classes: sources,
            },
        }
    );
    assert_eq!(
        settings.evidence,
        EvidenceSettings {
            parent_chain_order: None,
            expansion: ExpansionMode::RelevantBlocks,
            evidence_counter: CounterMode::Utf8AnswerBound,
        }
    );
    assert_eq!(
        settings.search_budget,
        RequestBudget {
            k: 12,
            evidence_bytes: 4000,
            deadline_ms: 9000
        }
    );
    assert_eq!(
        settings.ask_budget,
        AskBudget {
            k: 7,
            evidence_bytes: 3000,
            search_deadline_ms: 8000,
            output_tokens: Some(512)
        }
    );
    assert_eq!(settings.model, "qwen3-8b");
    assert_eq!(
        settings.prompt,
        AnswerPrompt::Presented {
            version: PromptVersion::ProcedureFirst,
            presentation: Presentation {
                language: Some("fr-CA".to_owned()),
                tone: Tone::Detailed,
            },
        }
    );
    assert_eq!(settings.chunk_profile, ChunkProfile::Structural);
}

#[test]
fn every_registered_setting_is_read_by_a_consumer() {
    let scratch = Scratch::new();
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
        let session = scratch.session(flags);
        read.extend(KnowledgeSettings::read(&session.resolved()).unwrap().1);
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
    assert_eq!(read.union(&catalog).count(), read.len() + catalog.len());
}

#[test]
fn compute_off_switches_off_every_model_stage_and_keeps_the_code_routes() {
    let scratch = Scratch::new();
    let settings = scratch
        .session(&["models.compute=off", "search.intent.expansion=hyde"])
        .knowledge()
        .unwrap();
    assert_eq!(settings.compute, Compute::Off);
    assert_eq!(settings.search.intent_expansion, IntentExpansion::Off);
    assert!(!settings.search.dense_enabled);
    assert!(!settings.search.rerank_enabled);
    assert!(settings.search.lexical_enabled);
    assert!(settings.search.identifier_enabled);
    assert!(!settings.search.identifier_noise_guard);
    assert!(settings.search.structured_enabled);
    let refused = Session::at(
        &scratch.config(),
        None,
        Some(&scratch.home()),
        &["models.compute=cpu".to_owned()],
    )
    .unwrap_err()
    .to_string();
    assert_eq!(refused, "--set models.compute=cpu: cpu mode comes after M1");
}

#[test]
fn the_flag_then_the_project_then_the_user_file_win() {
    let scratch = Scratch::new();
    scratch.user("tone = \"brief\"\nlanguage = \"es\"\nsearch.k = 3\n");
    let project = scratch.project("tone = \"normal\"\nlanguage = \"fr\"\n");
    let session = scratch.session(&["tone=detailed"]);
    let resolved = session.resolved();
    assert_eq!(resolved.get("tone").unwrap().source, Source::Flag);
    assert_eq!(
        resolved.get("language").unwrap().source,
        Source::File {
            layer: LayerName::Project,
            path: project.clone(),
        }
    );
    assert_eq!(
        resolved.get("search.k").unwrap().source,
        Source::File {
            layer: LayerName::User,
            path: scratch.config().join(USER_FILE),
        }
    );
    assert_eq!(session.files.project.as_deref(), Some(project.as_path()));
    let without_project = Session::at(&scratch.config(), None, Some(&scratch.home()), &[]).unwrap();
    assert_eq!(without_project.files.project, None);
    assert_eq!(without_project.resolved().text("language"), Some("es"));
}

#[test]
fn the_mcp_session_reads_a_project_only_through_an_explicit_workspace_inside_home() {
    let scratch = Scratch::new();
    scratch.project("tone = \"brief\"\n");
    let plain = Session::for_mcp_at(&scratch.config(), None, Some(&scratch.home()), &[]).unwrap();
    assert_eq!(plain.resolved().text("tone"), Some("normal"));
    let workspace = Session::for_mcp_at(
        &scratch.config(),
        Some(&scratch.home().join("work")),
        Some(&scratch.home()),
        &[],
    )
    .unwrap();
    assert_eq!(workspace.resolved().text("tone"), Some("brief"));
    let outside = Session::for_mcp_at(
        &scratch.config(),
        Some(&scratch.0),
        Some(&scratch.home()),
        &[],
    )
    .unwrap_err()
    .to_string();
    assert_eq!(
        outside,
        format!(
            "--workspace {}: the directory is outside the home directory: no project file is read",
            scratch.0.display()
        )
    );
}

#[test]
fn the_mcp_session_refuses_a_workspace_that_is_not_a_directory() {
    let scratch = Scratch::new();
    scratch.project("tone = \"brief\"\n");
    let readme = scratch.home().join("work").join("README.md");
    fs::write(&readme, "# work\n").unwrap();
    let refused = Session::for_mcp_at(&scratch.config(), Some(&readme), Some(&scratch.home()), &[])
        .unwrap_err()
        .to_string();
    assert_eq!(
        refused,
        format!(
            "--workspace {}: the path is not a directory: no project file is read",
            readme.display()
        )
    );
}

#[test]
fn a_refused_file_is_named_and_stops_the_session() {
    let scratch = Scratch::new();
    scratch.user("[access]\nread = []\n");
    let error = Session::at(&scratch.config(), None, Some(&scratch.home()), &[])
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        format!(
            "{}: unknown key \"access\"",
            scratch.config().join(USER_FILE).display()
        )
    );
}

#[test]
fn parent_chain_settings_round_trip_and_refuse_legacy_order() {
    let scratch = Scratch::new();
    scratch.user(concat!(
        "[evidence]\nexpansion = \"parent_chain\"\n",
        "parent_chain_order = \"largest_fitting_parent\"\n"
    ));
    let evidence = scratch.session(&[]).knowledge().unwrap().evidence;
    assert_eq!(evidence.expansion, ExpansionMode::ParentChain);
    assert_eq!(
        serde_json::to_value(evidence).unwrap()["parent_chain_order"],
        "largest_fitting_parent"
    );
    scratch.user(concat!(
        "[evidence]\nexpansion = \"full_section\"\n",
        "parent_chain_order = \"largest_fitting_parent\"\n"
    ));
    assert!(matches!(
        scratch.session(&[]).knowledge(),
        Err(Failure::Refused(_))
    ));
}
