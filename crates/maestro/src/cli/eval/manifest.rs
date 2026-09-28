//! The ladder's manifest, `maestro-ladder-manifest/1`: the suite, the
//! collection, the warm-ups, the output directory and the rungs, each a named
//! search configuration and whether `ask` runs, with which settings. It is private: it names the
//! owner's files. Relative paths resolve from the manifest's directory.

use super::{
    graph_manifest::{GraphManifest, Inputs},
    graph_output::Code,
    private_run::CheckedRun,
    rung_prompt::RungPrompt,
};
use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::{answer::AskBudget, search::SearchConfiguration};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
};

/// The contract a manifest follows.
const SCHEMA: &str = "maestro-ladder-manifest/1";
/// The most fused candidates a rung may rerank, as search accepts.
const MAX_RERANK_DEPTH: usize = 120;
/// The longest rung name, which names the rung's report files.
const MAX_NAME_BYTES: usize = 64;

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
    /// Optional private graph-check manifest binding labels, authority and approval.
    #[serde(default)]
    #[serde(rename = "graph_manifest")]
    pub(super) graph: Option<PathBuf>,
    /// The rungs, run in this order.
    pub(super) rungs: Vec<Rung>,
}

/// One configuration of the ladder.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rung {
    /// Its name: lower-case letters, digits and dashes, which name its files.
    pub(super) name: String,
    /// How its searches, and the searches of its asks, run.
    pub(super) configuration: RungConfiguration,
    /// How each question also runs through `ask`; `None` when it does not.
    /// The manifest writes `false`, `true` for the default settings, or the
    /// settings.
    #[serde(serialize_with = "write_ask", deserialize_with = "read_ask")]
    pub(super) ask: Option<AskSettings>,
}

/// A rung's `ask` settings; each one absent is `ask`'s default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::min_ident_chars,
    reason = "ask's budget names this limit k, as the manifest does"
)]
pub(super) struct AskSettings {
    /// The passages given to the answerer.
    pub(super) k: Option<u32>,
    /// The evidence budget, in UTF-8 bytes.
    pub(super) max_tokens: Option<u32>,
    /// The most tokens each answerer reply generates.
    pub(super) output_tokens: Option<u32>,
    /// The answer prompt: a version, or a private prompt file.
    pub(super) prompt: RungPrompt,
    /// The SHA-256 digest, in hexadecimal, of the registered answerer card
    /// the rung asks with; absent, the latest registered non-thinking
    /// answerer of the default model.
    pub(super) card: Option<String>,
}

impl AskSettings {
    /// The budget `ask` runs under: [`AskBudget::default`] with these
    /// settings.
    pub(super) fn budget(&self) -> AskBudget {
        let default = AskBudget::default();
        AskBudget {
            k: self.k.unwrap_or(default.k),
            max_tokens: self.max_tokens.unwrap_or(default.max_tokens),
            output_tokens: self.output_tokens.unwrap_or(default.output_tokens),
            ..default
        }
    }

    /// The digest of the answerer card the rung names, if any.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] for a card that is not a SHA-256 digest.
    pub(super) fn answerer_card(&self) -> Result<Option<Digest>, Failure> {
        self.card
            .as_deref()
            .map(|card| {
                Digest::parse(card)
                    .map_err(|_| Failure::refused("a rung's answerer card is not a SHA-256 digest"))
            })
            .transpose()
    }
}

/// Reads a rung's `ask`: `false`, `true` for the default settings, or its
/// settings.
fn read_ask<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<AskSettings>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Bool(asks) => Ok(asks.then(AskSettings::default)),
        value => AskSettings::deserialize(value)
            .map(Some)
            .map_err(de::Error::custom),
    }
}

/// Writes a rung's `ask` as [`read_ask`] reads it.
#[expect(
    clippy::ref_option,
    reason = "serde's `serialize_with` passes the field by reference"
)]
fn write_ask<S: Serializer>(ask: &Option<AskSettings>, serializer: S) -> Result<S::Ok, S::Error> {
    match ask {
        None => serializer.serialize_bool(false),
        Some(settings) if *settings == AskSettings::default() => serializer.serialize_bool(true),
        Some(settings) => settings.serialize(serializer),
    }
}

/// Closed graph rung selection, separate from the unchanged S1 route configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum GraphSelection {
    /// Unchanged passage routes.
    #[default]
    None,
    /// Ladybug-only diagnostic, pending G13/G27.
    Ladybug,
    /// Passage and Ladybug pairing, pending G13/G27.
    Pairing,
}

/// A rung's search configuration, as the manifest writes it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RungConfiguration {
    /// The routes that run.
    pub(super) routes: Routes,
    /// The reciprocal rank fusion constant K.
    pub(super) rrf_k: NonZeroU32,
    /// Each route's weight in fusion.
    pub(super) weights: Weights,
    /// The reranker and its depth, absent when reranking is off.
    pub(super) rerank: Option<Rerank>,
    /// The least top reranker score `ask` answers from; absent, or when
    /// rerank does not run, `ask` answers whatever the score. JSON holds no
    /// non-finite number, and parsing refuses one beyond `f32`.
    pub(super) min_rerank_score: Option<f32>,
}

/// Which routes run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each route has its own switch, as search's configuration does"
)]
pub(super) struct Routes {
    /// Graph selection; enabled adapters are refused until G13/G27 land.
    #[serde(default)]
    pub(super) graph: GraphSelection,
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rerank {
    /// The SHA-256 digest of its card, in hexadecimal.
    pub(super) card: String,
    /// The fused candidates it reranks.
    pub(super) depth: NonZeroUsize,
}

impl RungConfiguration {
    /// The configuration search runs under.
    pub(super) fn search(&self) -> SearchConfiguration {
        SearchConfiguration {
            dense_enabled: self.routes.dense,
            lexical_enabled: self.routes.lexical,
            identifier_enabled: self.routes.identifier,
            structured_enabled: self.routes.structured,
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
        let path = fs::canonicalize(path).map_err(|error| Failure::refused_by(&error))?;
        let base = path.parent().unwrap_or_else(|| Path::new(""));
        let manifest = Self::parse(&text, base)?;
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
        let private = manifest
            .graph
            .as_ref()
            .map(|path| GraphManifest::read(&base.join(path)).map_err(Code::failure))
            .transpose()?;
        if let Some(inputs) = &private {
            if inputs.collection != manifest.collection {
                return Err(Code::Isolation.failure());
            }
            manifest.suite = inputs
                .run
                .input(&base.join(&manifest.suite))
                .map_err(Code::failure)?;
            manifest.output = inputs
                .run
                .directory(&base.join(&manifest.output))
                .map_err(Code::failure)?;
            CheckedRun::local_router().map_err(Code::failure)?;
        }
        manifest.graph = manifest.graph.map(|path| base.join(path));
        manifest.suite = base.join(&manifest.suite);
        manifest.output = base.join(&manifest.output);
        for rung in &mut manifest.rungs {
            if let Some(AskSettings {
                prompt: RungPrompt::File(file),
                ..
            }) = &mut rung.ask
            {
                check_prompt_path(private.as_ref(), &base.join(&file.file))?;
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
    if configuration.routes.graph != GraphSelection::None {
        return Err(Code::GraphUnavailable.failure());
    }
    let routes = configuration.routes;
    if !(routes.dense || routes.lexical || routes.identifier || routes.structured) {
        return Err(Failure::refused(format!(
            "the rung `{}` runs no route",
            rung.name
        )));
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
    if let Some(settings) = &rung.ask {
        if !settings.budget().is_within_limits() {
            return Err(Failure::refused(format!(
                "the rung `{}` has ask settings outside ask's limits",
                rung.name
            )));
        }
        settings.answerer_card()?;
    }
    configuration.reranker().map(drop)
}

/// Private prompts pass the same path guard before their text is read.
fn check_prompt_path(inputs: Option<&Inputs>, path: &Path) -> Result<(), Failure> {
    if let Some(inputs) = inputs {
        inputs.run.input(path).map_err(Code::failure)?;
    }
    Ok(())
}
