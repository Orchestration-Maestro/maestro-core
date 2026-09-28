//! The ladder's manifest, `maestro-ladder-manifest/1`: the suite, the
//! collection, the warm-ups, the output directory and the rungs, each a named
//! search configuration and whether `ask` runs. It is private: it names the
//! owner's files. Relative paths resolve from the manifest's directory.

use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::search::SearchConfiguration;
use serde::{Deserialize, Serialize};
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
    /// Whether each question also runs through `ask`.
    pub(super) ask: bool,
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
    /// manifest.
    pub(super) fn parse(text: &str, base: &Path) -> Result<Self, Failure> {
        let mut manifest: Self = serde_json::from_str(text)
            .map_err(|error| Failure::refused(format!("the manifest is not {SCHEMA}: {error}")))?;
        manifest.check()?;
        manifest.suite = base.join(&manifest.suite);
        manifest.output = base.join(&manifest.output);
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
    configuration.reranker().map(drop)
}
