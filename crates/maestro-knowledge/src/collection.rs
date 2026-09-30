//! A collection's declaration: `maestro-collection/1`, the strict JSON that
//! declares a collection, the profiles that process it and its sources
//! (docs/architecture/01 §1, ADR-0014).
//!
//! Strict means that the text is one JSON object with no number out of range;
//! every object the contract names is a JSON object, never an array of its
//! values; every key is one the contract names and appears once in its object;
//! every value is one the contract allows, a named one written as a string;
//! every path stays inside its directory ([`RelativePath`]); every id is a
//! name ([`maestro_kernel::scope::check_collection_name`] for the collection,
//! [`maestro_kernel::scope::check_name`] for each source), so they form scope
//! paths; and no two sources share an id. The files a
//! declaration names, its quality ledger and the directory of its evaluation
//! suites, are checked when first read, so they may not exist yet.
//!
//! A dangling reference, a name the declaration uses without defining it,
//! cannot occur in this version: no key refers to a name the declaration
//! defines. A manifest's binding resolves in the machine's bindings
//! ([`Declaration::manifest_paths`]) and a profile in the stage that runs it.
//! The check arrives with the first key that does refer to a declared name,
//! in the source policies of S6.

use crate::{relative_path::RelativePath, shape, strict_json};
use maestro_kernel::artifact::Digest;
use maestro_kernel::binding::{self, Bindings};
use serde::{Deserialize, Serialize, de};
use std::{collections::BTreeSet, error, fmt, path::PathBuf, str::FromStr};

/// A collection's declaration. [`str::parse`] reads one from a JSON object
/// only, and refuses two sources that share an id, which the shape alone
/// allows.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "WireDeclaration", into = "WireDeclaration")]
#[non_exhaustive]
pub struct Declaration {
    /// The contract the declaration follows.
    pub schema: Schema,
    /// The collection's id, such as `ctm`: a collection name.
    pub id: String,
    /// What the collection holds, for people.
    pub title: String,
    /// Who may see what the collection derives: a scope tag on every record.
    pub visibility: Visibility,
    /// The profiles that process every source.
    pub profiles: Profiles,
    /// The collection's quality ledger.
    pub quality: Quality,
    /// Where the collection's documents come from, in the declared order.
    pub sources: Vec<Source>,
    /// The collection's evaluation suites.
    pub evals: Evals,
    /// Exact source-policy identity; absent for S1-only declarations.
    pub source_policy: Option<PolicyReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Version-sensitive strict collection fields before link validation.
struct WireDeclaration {
    /// The contract the declaration follows.
    #[serde(deserialize_with = "shape::name")]
    schema: Schema,
    /// The collection's id, such as `ctm`: a collection name.
    #[serde(deserialize_with = "shape::collection_id")]
    id: String,
    /// What the collection holds, for people.
    title: String,
    /// Who may see what the collection derives: a scope tag on every record.
    #[serde(deserialize_with = "shape::name")]
    visibility: Visibility,
    /// The profiles that process every source.
    #[serde(deserialize_with = "shape::object")]
    profiles: Profiles,
    /// The collection's quality ledger.
    #[serde(deserialize_with = "shape::object")]
    quality: Quality,
    /// Where the collection's documents come from, in the declared order.
    #[serde(deserialize_with = "shape::objects")]
    sources: Vec<Source>,
    /// The collection's evaluation suites.
    #[serde(deserialize_with = "shape::object")]
    evals: Evals,
    /// Presence distinguishes an omitted v1 link from explicit v2 null.
    #[serde(
        default,
        deserialize_with = "policy_link",
        skip_serializing_if = "Link::is_missing"
    )]
    source_policy: Link,
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
        let value = strict_json::bounded(text.as_bytes()).map_err(Error::Json)?;
        if value.get("schema").and_then(serde_json::Value::as_str) != Some("maestro-collection/1") {
            strict_json::strings(&value).map_err(Error::Json)?;
        }
        let declaration: Self = strict_json::object(value).map_err(Error::Json)?;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Schema {
    /// `maestro-collection/1`.
    #[serde(rename = "maestro-collection/1")]
    V1,
    /// `maestro-collection/2`, with a required nullable source-policy link.
    #[serde(rename = "maestro-collection/2")]
    V2,
}

/// Who may see what a collection derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// `public`: anyone.
    Public,
    /// `private`: only the principals granted its scope.
    Private,
}

/// The profiles that process every source of a collection, each named with
/// its version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Quality {
    /// The ledger, relative to the declaration's directory.
    pub ledger: RelativePath,
}

/// Where a collection keeps its evaluation suites.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Evals {
    /// The directory of the collection's suites, relative to the
    /// declaration's: each `<name>.jsonl` in it is the suite `<name>`.
    pub suite: RelativePath,
}

/// A declared origin of a collection's documents.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// `import`: from an existing corpus, through its manifest.
    Import,
}

/// When a source is brought up to date: an owner's decision, never widened by
/// automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

/// A source-policy resource identity, not a path or an access grant.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyReference {
    /// Logical resource identifier, validated by the source-policy resolver.
    #[serde(deserialize_with = "resource_id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub id: String,
    /// Exact immutable SHA-256 resource digest.
    #[schemars(with = "String", regex(pattern = "^[0-9a-f]{64}$"))]
    pub digest: Digest,
}

/// A version-sensitive nullable link on the wire.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum Link {
    /// The v1 contract has no link field.
    #[default]
    Missing,
    /// The v2 contract requires a field, including explicit null.
    Present(Option<PolicyReference>),
}

impl Link {
    /// Omitted only for v1 serialization.
    fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl Serialize for Link {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Missing => serializer.serialize_none(),
            Self::Present(value) => value.serialize(serializer),
        }
    }
}

/// A present link is null or an object, never an array of fields.
fn policy_link<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Link, D::Error> {
    #[derive(Deserialize)]
    struct Object(#[serde(deserialize_with = "shape::object")] PolicyReference);
    Option::<Object>::deserialize(deserializer)
        .map(|value| Link::Present(value.map(|Object(reference)| reference)))
}

impl TryFrom<WireDeclaration> for Declaration {
    type Error = &'static str;
    fn try_from(wire: WireDeclaration) -> Result<Self, Self::Error> {
        let source_policy = match (wire.schema, wire.source_policy) {
            (Schema::V1, Link::Missing) => None,
            (Schema::V2, Link::Present(reference)) => reference,
            _ => return Err("source_policy is forbidden in v1 and required in v2"),
        };
        Ok(Self {
            schema: wire.schema,
            id: wire.id,
            title: wire.title,
            visibility: wire.visibility,
            profiles: wire.profiles,
            quality: wire.quality,
            sources: wire.sources,
            evals: wire.evals,
            source_policy,
        })
    }
}

impl From<Declaration> for WireDeclaration {
    fn from(value: Declaration) -> Self {
        let source_policy = match value.schema {
            Schema::V1 => Link::Missing,
            Schema::V2 => Link::Present(value.source_policy),
        };
        Self {
            schema: value.schema,
            id: value.id,
            title: value.title,
            visibility: value.visibility,
            profiles: value.profiles,
            quality: value.quality,
            sources: value.sources,
            evals: value.evals,
            source_policy,
        }
    }
}

/// Source-policy IDs use the plan's ASCII grammar, not filesystem paths.
fn resource_id<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    let id = String::deserialize(decoder)?;
    if !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        Ok(id)
    } else {
        Err(de::Error::custom("invalid source-policy resource ID"))
    }
}
