//! The ladder's manifest, `maestro-ladder-manifest/1`: the suite, the
//! collection, the warm-ups, the output directory and the rungs, each a named
//! search configuration and whether `ask` runs, with which settings. It is private: it names the
//! owner's files. Relative paths resolve from the manifest's directory.

use super::rank_settings::{Context, Prior, SourcePriorSetting};
use super::{ask_settings::read_ask, rung_prompt::RungPrompt};
use crate::failure::Failure;
use maestro_kernel::{artifact::Digest, evidence::RequestBudget};
use maestro_knowledge::search::{
    IntentExpansion, IntentTrigger, SearchConfiguration, SourcePrior, StageWindow,
    evidence::{EvidenceSettings, ExpansionMode, ParentChainOrder},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::BTreeSet,
    fs,
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) use super::ask_settings::AskSettings;

/// The contract a manifest follows.
const SCHEMA: &str = "maestro-ladder-manifest/1";
/// The most fused candidates a rung may rerank, as search accepts.
const MAX_RERANK_DEPTH: usize = 120;
/// The longest rung name, which names the rung's report files.
const MAX_NAME_BYTES: usize = 64;
/// The name of the comparison's files in the output directory, which no rung
/// may take.
pub(super) const COMPARISON_NAME: &str = "ladder";

/// A checked ladder manifest.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    /// The contract it follows, `maestro-ladder-manifest/1`.
    schema: String,
    /// The `maestro-suite/1` file of the questions.
    pub(super) suite: PathBuf,
    /// The collection whose published generation every rung searches.
    pub(super) collection: String,
    /// The questions each rung runs first, from the start of the suite,
    /// unscored.
    pub(super) warm_ups: usize,
    /// The directory the reports go to, new or empty.
    pub(super) output: PathBuf,
    /// The rungs, run in this order.
    pub(super) rungs: Vec<Rung>,
}

/// One configuration of the ladder.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rung {
    /// Its name: lower-case letters, digits and dashes, which name its files.
    pub(super) name: String,
    /// How its searches, and the searches of its asks, run.
    pub(super) configuration: RungConfiguration,
    /// How each question also runs through `ask`; `None` when it does not.
    /// The manifest writes `false`, `true` for the default settings, or the
    /// settings.
    #[serde(deserialize_with = "read_ask")]
    pub(super) ask: Option<AskSettings>,
    /// Search-only evidence budget; absent uses the default search budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) search_budget: Option<RequestBudget>,
}

/// A rung's search configuration, as the manifest writes it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RungConfiguration {
    /// Optional evidence expansion; absent retains `full_section` behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) evidence_expansion: Option<ExpansionMode>,
    /// Optional admission order, valid only for parent-chain expansion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) parent_chain_order: Option<ParentChainOrder>,
    /// Optional hypothetical-document retrieval; absent means off.
    #[serde(default)]
    pub(super) intent_expansion: IntentExpansion,
    /// Run beside originals or only after a weak first ranking.
    #[serde(default)]
    pub(super) intent_trigger: IntentTrigger,
    /// Explicit registered answerer-role card for expansion, independent of ask.
    #[serde(default)]
    pub(super) intent_card: Option<String>,
    /// Maximum expansion model duration in milliseconds.
    #[serde(default = "default_intent_deadline")]
    pub(super) intent_deadline_ms: u32,
    /// RRF weight of each additional intent route.
    #[serde(
        default,
        deserialize_with = "read_intent_weight",
        serialize_with = "write_intent_weight"
    )]
    pub(super) intent_weight: Option<f64>,
    /// The most candidates intent votes add to the rerank beyond its depth.
    #[serde(default = "default_intent_rerank_additions")]
    pub(super) intent_rerank_additions: usize,
    /// Maximum dense, lexical and intent candidates.
    #[serde(default = "default_routes_limit")]
    pub(super) routes_limit: usize,
    /// Identifier candidates, additionally bounded by `routes_limit`.
    #[serde(default = "default_identifier_limit")]
    pub(super) identifier_limit: usize,
    /// Maximum candidates retained by fusion, at most 120.
    #[serde(default = "default_fusion_pool")]
    pub(super) fusion_pool: usize,
    /// The routes that run.
    pub(super) routes: Routes,
    /// Whether the identifier route drops identifiers too common to rank,
    /// and fusion takes no hits from an unavailable route; absent, off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) identifier_noise_guard: bool,
    /// The reciprocal rank fusion constant K.
    pub(super) rrf_k: NonZeroU32,
    /// Each route's weight in fusion.
    pub(super) weights: Weights,
    /// The reranker and its depth, absent when reranking is off.
    pub(super) rerank: Option<Rerank>,
    /// The least top reranker score `ask` answers from; absent, `ask`
    /// answers whatever the score, and a rung that sets it must rerank. JSON
    /// holds no non-finite number, and parsing refuses one beyond `f32`,
    /// through the `float_roundtrip` feature of `serde_json`.
    pub(super) min_rerank_score: Option<f32>,
    /// Optional configured section-class penalty.
    #[serde(default)]
    pub(super) section_prior: Prior,
    /// A fixed route window, in milliseconds, for an experiment; absent,
    /// the routes' windows derive from the search deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) stage_window_ms: Option<NonZeroU32>,
    /// The source prior; absent, search's default, official-first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) source_prior: Option<SourcePriorSetting>,
}

/// Which routes run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each route has its own switch, as search's configuration does"
)]
pub(super) struct Routes {
    /// The dense route.
    pub(super) dense: bool,
    /// The lexical route.
    pub(super) lexical: bool,
    /// The identifier route.
    pub(super) identifier: bool,
    /// The structured route, for global questions.
    pub(super) structured: bool,
}

/// Each route's weight in fusion.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Weights {
    /// The dense route's.
    pub(super) dense: f64,
    /// The lexical route's.
    pub(super) lexical: f64,
    /// The identifier route's.
    pub(super) identifier: f64,
    /// The structured route's.
    pub(super) structured: f64,
}

/// The reranker a rung runs, registered in the collection but not
/// necessarily selected.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rerank {
    /// The SHA-256 digest of its card, in hexadecimal.
    pub(super) card: String,
    /// The fused candidates it reranks.
    pub(super) depth: NonZeroUsize,
    /// Fused-position weight in a rank fusion with the reranked position,
    /// using the rung's `rrf_k`; absent for unchanged rerank order.
    pub(super) blend: Option<f32>,
    /// Maximum final demotion of a fused top-ten candidate.
    pub(super) demotion_cap: Option<u16>,
    /// Reranker-only source context.
    #[serde(default)]
    pub(super) candidate_context: Context,
}

/// Default bounded expansion deadline.
fn default_intent_deadline() -> u32 {
    SearchConfiguration::default().intent_deadline_ms
}

/// Default per-route candidate limit.
fn default_routes_limit() -> usize {
    SearchConfiguration::DEFAULT_ROUTES_LIMIT
}

/// Default identifier-route candidate cap.
fn default_identifier_limit() -> usize {
    SearchConfiguration::DEFAULT_IDENTIFIER_LIMIT
}

/// Default fusion pool size.
fn default_fusion_pool() -> usize {
    SearchConfiguration::MAX_FUSION_POOL
}

/// A present manifest weight must be a number; an omitted weight stays unset until search builds.
fn read_intent_weight<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    f64::deserialize(deserializer).map(Some)
}

/// Preserve the serialized default while keeping `SearchConfiguration` its single source.
#[expect(
    clippy::ref_option,
    reason = "serde serialize_with requires a reference to the serialized field type"
)]
fn write_intent_weight<S>(weight: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    weight
        .unwrap_or_else(|| SearchConfiguration::default().intent_weight)
        .serialize(serializer)
}

/// Default number of intent additions to the rerank.
fn default_intent_rerank_additions() -> usize {
    SearchConfiguration::default().intent_rerank_additions
}

impl Rung {
    /// Resolved request budget and evidence settings shared by execution and reporting.
    pub(super) fn resolved_search_settings(&self) -> (RequestBudget, EvidenceSettings) {
        if let Some(ask) = &self.ask {
            return (ask.budget().into(), ask.evidence());
        }
        (
            self.search_budget.unwrap_or(RequestBudget {
                k: 5,
                evidence_bytes: 6_000,
                deadline_ms: RequestBudget::MAX_DEADLINE_MS,
            }),
            self.configuration.evidence(),
        )
    }
}

impl RungConfiguration {
    /// Search-only delivery settings, with legacy defaults when the fields are absent.
    pub(super) fn evidence(&self) -> EvidenceSettings {
        EvidenceSettings {
            expansion: self
                .evidence_expansion
                .unwrap_or(ExpansionMode::FullSection),
            parent_chain_order: self.parent_chain_order,
            ..EvidenceSettings::default()
        }
    }

    /// The configuration search runs under.
    #[expect(
        clippy::expect_used,
        reason = "manifest validation checks both priors before execution"
    )]
    pub(super) fn search(&self) -> SearchConfiguration {
        SearchConfiguration {
            intent_expansion: self.intent_expansion,
            intent_trigger: self.intent_trigger,
            intent_deadline_ms: self.intent_deadline_ms,
            intent_weight: self
                .intent_weight
                .unwrap_or_else(|| SearchConfiguration::default().intent_weight),
            intent_rerank_additions: self.intent_rerank_additions,
            routes_limit: self.routes_limit,
            identifier_limit: self.identifier_limit,
            fusion_pool: self.fusion_pool,
            dense_enabled: self.routes.dense,
            lexical_enabled: self.routes.lexical,
            identifier_enabled: self.routes.identifier,
            structured_enabled: self.routes.structured,
            identifier_noise_guard: self.identifier_noise_guard,
            rrf_k: self.rrf_k,
            dense_weight: self.weights.dense,
            lexical_weight: self.weights.lexical,
            identifier_weight: self.weights.identifier,
            structured_weight: self.weights.structured,
            rerank_enabled: self.rerank.is_some(),
            rerank_depth: self
                .rerank
                .as_ref()
                .map_or(SearchConfiguration::default().rerank_depth, |rerank| {
                    rerank.depth
                }),
            min_rerank_score: self.min_rerank_score,
            rerank_blend: self.rerank.as_ref().and_then(|rerank| rerank.blend),
            rerank_demotion_cap: self.rerank.as_ref().and_then(|rerank| rerank.demotion_cap),
            candidate_context: self
                .rerank
                .as_ref()
                .map_or_default(|rerank| rerank.candidate_context.search()),
            section_prior: self
                .section_prior
                .search()
                .expect("validated section prior"),
            stage_window: self.stage_window_ms.map_or(StageWindow::Derived, |window| {
                StageWindow::Fixed(Duration::from_millis(u64::from(window.get())))
            }),
            source_prior: self
                .source_prior
                .as_ref()
                .map_or_else(SourcePrior::default, |prior| {
                    prior.search().expect("validated source prior")
                }),
        }
    }

    /// The digest of the reranker's card, when reranking is on.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] for a card that is not a SHA-256 digest.
    pub(super) fn reranker(&self) -> Result<Option<Digest>, Failure> {
        self.rerank
            .as_ref()
            .map(|rerank| {
                Digest::parse(&rerank.card)
                    .map_err(|_| Failure::refused("a rung's reranker card is not a SHA-256 digest"))
            })
            .transpose()
    }
}

impl Manifest {
    /// The manifest at `path`, checked, its paths resolved from its
    /// directory.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] for a file that cannot be read, is not a
    /// `maestro-ladder-manifest/1` manifest, or names an output directory that
    /// holds files.
    pub(super) fn read(path: &Path) -> Result<Self, Failure> {
        let text = fs::read_to_string(path)
            .map_err(|error| Failure::refused(format!("cannot read the manifest: {error}")))?;
        let base = path.parent().unwrap_or_else(|| Path::new(""));
        let manifest = Self::parse(&text, base)?;
        if manifest.output.exists() && !manifest.output.is_dir() {
            return Err(Failure::refused(
                "the manifest's output path is not a directory",
            ));
        }
        let occupied =
            fs::read_dir(&manifest.output).is_ok_and(|mut entries| entries.next().is_some());
        if occupied {
            return Err(Failure::refused(
                "the manifest's output directory already holds files",
            ));
        }
        Ok(manifest)
    }

    /// The manifest `text` holds, checked, its paths resolved from `base`.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] for text that is not a `maestro-ladder-manifest/1`
    /// manifest, and for a rung's prompt file that cannot be read or is not
    /// a prompt.
    pub(super) fn parse(text: &str, base: &Path) -> Result<Self, Failure> {
        let mut manifest: Self = serde_json::from_str(text)
            .map_err(|error| Failure::refused(format!("the manifest is not {SCHEMA}: {error}")))?;
        manifest.check()?;
        manifest.suite = base.join(&manifest.suite);
        manifest.output = base.join(&manifest.output);
        for rung in &mut manifest.rungs {
            if let Some(AskSettings {
                prompt: RungPrompt::File(file),
                ..
            }) = &mut rung.ask
            {
                file.read(base, &rung.name)?;
            }
        }
        Ok(manifest)
    }

    /// Refuses what the manifest's shape alone allows.
    fn check(&self) -> Result<(), Failure> {
        if self.schema != SCHEMA {
            return Err(Failure::refused(format!(
                "the manifest's schema is not {SCHEMA}"
            )));
        }
        if self.collection.trim().is_empty() {
            return Err(Failure::refused("the manifest names no collection"));
        }
        if self.rungs.is_empty() {
            return Err(Failure::refused("the manifest holds no rung"));
        }
        let mut names = BTreeSet::new();
        for rung in &self.rungs {
            if rung.name == COMPARISON_NAME {
                return Err(Failure::refused(format!(
                    "the rung name `{COMPARISON_NAME}` names the comparison's files"
                )));
            }
            if !names.insert(rung.name.as_str()) {
                return Err(Failure::refused(format!(
                    "the rung name `{}` is given twice",
                    rung.name
                )));
            }
            check_rung(rung)?;
        }
        Ok(())
    }
}

/// Validates limits shared by search routes, fusion and reranking.
fn check_pipeline_limits(configuration: &RungConfiguration) -> Result<(), Failure> {
    if !(1..=SearchConfiguration::MAX_FUSION_POOL).contains(&configuration.routes_limit) {
        return Err(Failure::refused(
            "a rung's routes_limit must be between 1 and 120",
        ));
    }
    if !(1..=SearchConfiguration::MAX_FUSION_POOL).contains(&configuration.identifier_limit) {
        return Err(Failure::refused(
            "a rung's identifier_limit must be between 1 and 120",
        ));
    }
    if !(1..=SearchConfiguration::MAX_FUSION_POOL).contains(&configuration.fusion_pool) {
        return Err(Failure::refused(
            "a rung's fusion_pool must be between 1 and 120",
        ));
    }
    if configuration
        .rerank
        .as_ref()
        .is_some_and(|rerank| rerank.depth.get() > configuration.fusion_pool)
    {
        return Err(Failure::refused(
            "a rung's rerank depth cannot exceed its fusion pool",
        ));
    }
    Ok(())
}

/// Refuses a rung whose name cannot name a file or whose configuration search
/// would refuse.
fn check_rung(rung: &Rung) -> Result<(), Failure> {
    let name_is_safe = !rung.name.is_empty()
        && rung.name.len() <= MAX_NAME_BYTES
        && rung
            .name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !name_is_safe {
        return Err(Failure::refused(
            "a rung name is 1 to 64 lower-case letters, digits and dashes",
        ));
    }
    let configuration = &rung.configuration;
    if let Some(budget) = rung.search_budget
        && budget.evidence_bytes > RequestBudget::MAX_EVIDENCE_BUDGET
    {
        return Err(Failure::refused(format!(
            "the rung `{}` asks for {} evidence bytes, over the {}-byte ceiling",
            rung.name,
            budget.evidence_bytes,
            RequestBudget::MAX_EVIDENCE_BUDGET
        )));
    }
    if rung.ask.is_some() && rung.search_budget.is_some() {
        return Err(Failure::refused("search_budget requires ask false"));
    }
    check_intent(configuration)?;
    configuration
        .evidence()
        .validate()
        .map_err(|error| Failure::refused(error.to_string()))?;
    if rung.ask.is_some()
        && (configuration.evidence_expansion.is_some()
            || configuration.parent_chain_order.is_some())
    {
        return Err(Failure::refused(
            "search-only evidence settings require ask false",
        ));
    }
    check_pipeline_limits(configuration)?;
    let routes = configuration.routes;
    if !(routes.dense || routes.lexical || routes.identifier || routes.structured) {
        return Err(Failure::refused(format!(
            "the rung `{}` runs no route",
            rung.name
        )));
    }
    configuration.section_prior.search()?;
    configuration
        .source_prior
        .as_ref()
        .map(SourcePriorSetting::search)
        .transpose()?;
    if let Some(rerank) = &configuration.rerank {
        rerank.candidate_context.check()?;
        if rerank
            .blend
            .is_some_and(|weight| !weight.is_finite() || !(0.0..=1.0).contains(&weight))
        {
            return Err(Failure::refused("rerank blend must be between 0 and 1"));
        }
    }
    if !configuration.search().weights_are_valid() {
        return Err(Failure::refused(format!(
            "the rung `{}` has a weight that is negative or not finite",
            rung.name
        )));
    }
    if configuration
        .rerank
        .as_ref()
        .is_some_and(|rerank| rerank.depth.get() > MAX_RERANK_DEPTH)
    {
        return Err(Failure::refused(format!(
            "the rung `{}` reranks more than {MAX_RERANK_DEPTH} candidates",
            rung.name
        )));
    }
    if configuration.min_rerank_score.is_some() && configuration.rerank.is_none() {
        return Err(Failure::refused(format!(
            "the rung `{}` sets a relevance threshold but does not rerank",
            rung.name
        )));
    }
    if let Some(settings) = &rung.ask {
        settings.check_evidence_budget(&rung.name)?;
        if !settings.budget().is_within_limits() {
            return Err(Failure::refused(format!(
                "the rung `{}` has ask settings outside ask's limits",
                rung.name
            )));
        }
        settings.answerer_card()?;
        settings
            .evidence()
            .counter()
            .map_err(|error| Failure::refused(error.to_string()))?;
    }
    configuration.reranker().map(drop)
}

/// Checks the opt-in expansion card and bounded model deadline before a run.
fn check_intent(configuration: &RungConfiguration) -> Result<(), Failure> {
    if !configuration.intent_trigger.is_valid() {
        return Err(Failure::refused(
            "intent confidence threshold must be finite",
        ));
    }
    if !(1..=5000).contains(&configuration.intent_deadline_ms) {
        return Err(Failure::refused(
            "intent deadline must be between 1 and 5000 milliseconds",
        ));
    }
    if configuration.intent_rerank_additions > 120 {
        return Err(Failure::refused(
            "intent rerank additions must be at most 120",
        ));
    }
    if configuration.intent_expansion == IntentExpansion::Hyde
        && configuration.intent_card.is_none()
    {
        return Err(Failure::refused("hyde requires an explicit intent card"));
    }
    if let Some(card) = &configuration.intent_card {
        Digest::parse(card)
            .map_err(|_| Failure::refused("intent card must be a SHA-256 digest"))?;
    }
    Ok(())
}
