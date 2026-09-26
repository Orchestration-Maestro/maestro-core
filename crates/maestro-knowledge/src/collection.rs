//! A collection's declaration: `maestro-collection/1`, the strict JSON that
//! declares a collection, the profiles that process it and its sources
//! (docs/architecture/01 §1, ADR-0014).
//!
//! Strict means that the text is one JSON object with no number out of range;
//! every object the contract names is a JSON object, never an array of its
//! values; every key is one the contract names and appears once in its object;
//! every value is one the contract allows, a named one written as a string;
//! every path stays inside its directory ([`RelativePath`]); every id is a
//! scope name ([`maestro_kernel::scope::check_name`]), so the collection and
//! each source form a scope path; and no two sources share an id. The files a
//! declaration names, its quality ledger and its evaluation suite, are checked
//! when first read, so they may not exist yet.
//!
//! A dangling reference, a name the declaration uses without defining it,
//! cannot occur in this version: no key refers to a name the declaration
//! defines. A manifest's binding resolves in the machine's bindings
//! ([`Declaration::manifest_paths`]) and a profile in the stage that runs it.
//! The check arrives with the first key that does refer to a declared name,
//! in the source policies of S6.

use crate::{relative_path::RelativePath, shape};
use maestro_kernel::binding::{self, Bindings};
use serde::Deserialize;
use std::{collections::BTreeSet, error, fmt, path::PathBuf, str::FromStr};

/// A collection's declaration. [`str::parse`] reads one from a JSON object
/// only, and refuses two sources that share an id, which the shape alone
/// allows.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Declaration {
    /// The contract the declaration follows.
    #[serde(deserialize_with = "shape::name")]
    pub schema: Schema,
    /// The collection's id, such as `ctm`: a scope name.
    #[serde(deserialize_with = "shape::id")]
    pub id: String,
    /// What the collection holds, for people.
    pub title: String,
    /// Who may see what the collection derives: a scope tag on every record.
    #[serde(deserialize_with = "shape::name")]
    pub visibility: Visibility,
    /// The profiles that process every source.
    #[serde(deserialize_with = "shape::object")]
    pub profiles: Profiles,
    /// The collection's quality ledger.
    #[serde(deserialize_with = "shape::object")]
    pub quality: Quality,
    /// Where the collection's documents come from, in the declared order.
    #[serde(deserialize_with = "shape::objects")]
    pub sources: Vec<Source>,
    /// The collection's evaluation suite.
    #[serde(deserialize_with = "shape::object")]
    pub evals: Evals,
}

impl Declaration {
    /// Every source with the absolute path of its manifest, resolved through
    /// `bindings` before any work starts: all of them, or a refusal.
    ///
    /// # Errors
    ///
    /// [`binding::Error::Missing`], naming the binding, for the first source
    /// whose binding nothing binds.
    pub fn manifest_paths(
        &self,
        bindings: &Bindings,
    ) -> Result<Vec<(&Source, PathBuf)>, binding::Error> {
        self.sources
            .iter()
            .map(|source| {
                let root = bindings.path(&source.manifest.binding)?;
                Ok((source, source.manifest.path.under(root)))
            })
            .collect()
    }
}

impl FromStr for Declaration {
    type Err = Error;

    /// The declaration `text` holds.
    ///
    /// # Errors
    ///
    /// [`Error::Json`] when the text is not strict JSON of the contract's
    /// shape, and [`Error::DuplicateSource`] when two sources share an id.
    fn from_str(text: &str) -> Result<Self, Error> {
        let declaration: Self = shape::parse(text).map_err(Error::Json)?;
        match repeated_id(&declaration.sources) {
            Some(id) => Err(Error::DuplicateSource(id.to_owned())),
            None => Ok(declaration),
        }
    }
}

/// The first id that two of `sources` share.
fn repeated_id(sources: &[Source]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    sources
        .iter()
        .map(|source| source.id.as_str())
        .find(|id| !seen.insert(*id))
}

/// The contract a declaration follows; this version reads the first only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Schema {
    /// `maestro-collection/1`.
    #[serde(rename = "maestro-collection/1")]
    V1,
}

/// Who may see what a collection derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// `public`: anyone.
    Public,
    /// `private`: only the principals granted its scope.
    Private,
}

/// The profiles that process every source of a collection, each named with
/// its version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Profiles {
    /// How a captured document becomes Markdown (`technical-html/1`); it
    /// applies from S6, since S1 imports Markdown.
    pub extraction: String,
    /// How documents are cut into chunks (`structural-500-700/1`).
    pub chunking: String,
    /// The embedder, or the role whose model card is resolved at run time
    /// (`embed:winner`).
    pub embedding: String,
    /// The lexical analyzer (`bm25-en-fr/1`).
    pub sparse: String,
}

/// Where a collection keeps its quality ledger.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Quality {
    /// The ledger, relative to the declaration's directory.
    pub ledger: RelativePath,
}

/// Where a collection keeps its evaluation suite.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Evals {
    /// The suite, relative to the declaration's directory.
    pub suite: RelativePath,
}

/// A declared origin of a collection's documents.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Source {
    /// The source's id, unique in its collection, such as `docs-core`: a
    /// scope name.
    #[serde(deserialize_with = "shape::id")]
    pub id: String,
    /// How its documents arrive.
    #[serde(deserialize_with = "shape::name")]
    pub kind: SourceKind,
    /// When it is brought up to date.
    #[serde(deserialize_with = "shape::name")]
    pub sync: Synchronization,
    /// The corpus manifest it imports.
    #[serde(deserialize_with = "shape::object")]
    pub manifest: Manifest,
}

/// How a source's documents arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// `import`: from an existing corpus, through its manifest.
    Import,
}

/// When a source is brought up to date: an owner's decision, never widened by
/// automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Synchronization {
    /// `one-off`: once.
    OneOff,
    /// `manual`: when someone asks.
    Manual,
    /// `watch`: automatically, when the source changes.
    Watch,
}

/// Where a source's corpus manifest is: a path under a named binding, so the
/// declaration holds no machine path.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Manifest {
    /// The binding that names the directory, such as `corpus_root`.
    pub binding: String,
    /// The manifest, relative to that directory.
    pub path: RelativePath,
}

/// Why a text is not a `maestro-collection/1` declaration.
#[derive(Debug)]
pub enum Error {
    /// Not strict JSON of the contract's shape: not one JSON object, an array
    /// where the contract names an object, a number out of range, an unknown,
    /// repeated or missing key, a value the contract does not allow, a path
    /// that is not a [`RelativePath`] or an id that is not a scope name.
    Json(serde_json::Error),
    /// Two sources share this id.
    DuplicateSource(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(
                formatter,
                "not a strict maestro-collection/1 declaration: {error}"
            ),
            Self::DuplicateSource(id) => write!(formatter, "two sources share the id `{id}`"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::DuplicateSource(_) => None,
        }
    }
}
