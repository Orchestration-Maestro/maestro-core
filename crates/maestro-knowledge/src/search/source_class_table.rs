//! The default source classifier: a strict `maestro-source-classes/1` JSON
//! table of ordered rules. A rule matches on any of a document's
//! `source_kind`, its origin URL's host and its path prefix; the first rule
//! whose every given field matches names the class and label. No rule
//! matching leaves the document unclassified.
//!
//! The host is compared without case, past any login and port. The path
//! prefix is raw: compared as written, with case, not percent-decoded, and
//! not on whole segments. A URL's path starts with `/` and stops before its
//! query; a `corpus-path:` reference has no host, and its path is the
//! corpus path, without a leading `/`; any other reference is its own path.

use super::source_class::{Classification, SourceClass, SourceClassifier, SourceMetadata};
use crate::{
    corpus::{CORPUS_PATH, web_origin},
    shape,
};
use maestro_kernel::artifact::Digest;
use serde::Deserialize;
use std::{error, fmt, str};

/// The contract a table follows.
const SCHEMA: &str = "maestro-source-classes/1";

/// A table as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    /// `maestro-source-classes/1`.
    schema: String,
    /// The rules, the first match winning.
    #[serde(deserialize_with = "shape::objects")]
    rules: Vec<WrittenRule>,
}

/// A rule as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrittenRule {
    /// The exact `source_kind` it requires.
    source_kind: Option<String>,
    /// The origin host it requires, compared without case.
    host: Option<String>,
    /// The prefix the origin path must start with.
    path_prefix: Option<String>,
    /// The class it names.
    class: String,
    /// The label it gives.
    label: String,
}

/// A checked rule.
#[derive(Debug)]
struct Rule {
    /// The exact `source_kind` it requires.
    source_kind: Option<String>,
    /// The origin host it requires, compared without case.
    host: Option<String>,
    /// The prefix the origin path must start with.
    path_prefix: Option<String>,
    /// The class and label it names.
    classification: Classification,
}

impl Rule {
    /// The rule `rule` writes, once it names a known class, a one-line
    /// label and at least one matcher, none of them empty, and a host rule's
    /// path prefix starts with `/`.
    fn checked(rule: WrittenRule) -> Result<Self, TableError> {
        let class = SourceClass::named(&rule.class).ok_or(TableError::UnknownClass(rule.class))?;
        if rule.label.trim().is_empty() {
            return Err(TableError::BlankLabel);
        }
        if rule.label.chars().any(char::is_control) {
            return Err(TableError::ControlInLabel);
        }
        let matchers = [&rule.source_kind, &rule.host, &rule.path_prefix];
        if matchers.iter().all(|matcher| matcher.is_none()) {
            return Err(TableError::NoMatcher);
        }
        if matchers
            .iter()
            .any(|matcher| matcher.as_deref() == Some(""))
        {
            return Err(TableError::EmptyMatcher);
        }
        if rule.host.is_some()
            && rule
                .path_prefix
                .as_deref()
                .is_some_and(|prefix| !prefix.starts_with('/'))
        {
            return Err(TableError::RelativeHostPath);
        }
        Ok(Self {
            source_kind: rule.source_kind,
            host: rule.host,
            path_prefix: rule.path_prefix,
            classification: Classification {
                class,
                label: rule.label,
            },
        })
    }

    /// Whether each field it gives holds for `source`.
    fn matches(&self, source: &SourceMetadata<'_>, (host, path): (Option<&str>, &str)) -> bool {
        self.source_kind
            .as_deref()
            .is_none_or(|kind| source.source_kind == Some(kind))
            && self
                .host
                .as_deref()
                .is_none_or(|wanted| host.is_some_and(|host| host.eq_ignore_ascii_case(wanted)))
            && self
                .path_prefix
                .as_deref()
                .is_none_or(|prefix| path.starts_with(prefix))
    }
}

/// A source-class table and the digest of its bytes.
#[derive(Debug)]
pub struct SourceClassTable {
    /// Its rules, in order.
    rules: Vec<Rule>,
    /// The SHA-256 of the bytes it was read from.
    digest: Digest,
}

impl SourceClassTable {
    /// The table `bytes` hold.
    ///
    /// # Errors
    ///
    /// [`TableError`] when the bytes are not strict JSON of the contract, or
    /// a rule names an unknown class or a blank label.
    pub fn parse(bytes: &[u8]) -> Result<Self, TableError> {
        let text = str::from_utf8(bytes).map_err(|_| TableError::Encoding)?;
        let written: Written = shape::parse(text).map_err(TableError::Json)?;
        if written.schema != SCHEMA {
            return Err(TableError::Schema(written.schema));
        }
        let rules = written
            .rules
            .into_iter()
            .map(Rule::checked)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            rules,
            digest: Digest::of(bytes),
        })
    }

    /// The SHA-256 of the bytes it was read from, which reports echo.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }
}

impl SourceClassifier for SourceClassTable {
    fn classify(&self, source: &SourceMetadata<'_>) -> Option<Classification> {
        let origin = origin(source.source_ref);
        self.rules
            .iter()
            .find(|rule| rule.matches(source, origin))
            .map(|rule| rule.classification.clone())
    }
}

/// The host and path of `source_ref`: a web URL's host and path, or no
/// host and the corpus path of a `corpus-path:` reference; any other
/// reference has no host and is its own path.
fn origin(source_ref: &str) -> (Option<&str>, &str) {
    if let Some((host, path)) = web_origin(source_ref) {
        return (Some(host), path);
    }
    (
        None,
        source_ref.strip_prefix(CORPUS_PATH).unwrap_or(source_ref),
    )
}

/// Why bytes are not a source-class table.
#[derive(Debug)]
pub enum TableError {
    /// Not UTF-8.
    Encoding,
    /// Not strict JSON of the contract's shape.
    Json(serde_json::Error),
    /// A schema other than `maestro-source-classes/1`.
    Schema(String),
    /// A class the vocabulary does not have.
    UnknownClass(String),
    /// A label with no visible text.
    BlankLabel,
    /// A label holding a control character, such as a line break.
    ControlInLabel,
    /// A rule with no `source_kind`, `host` or `path_prefix`.
    NoMatcher,
    /// A rule with an empty `source_kind`, `host` or `path_prefix`.
    EmptyMatcher,
    /// A host rule whose path prefix does not start with `/`.
    RelativeHostPath,
}

impl fmt::Display for TableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding => formatter.write_str("the source-class table is not UTF-8"),
            Self::Json(error) => write!(formatter, "the source-class table is invalid: {error}"),
            Self::Schema(schema) => {
                write!(formatter, "unknown source-class table schema {schema:?}")
            }
            Self::UnknownClass(class) => write!(formatter, "unknown source class {class:?}"),
            Self::BlankLabel => formatter.write_str("a source-class label is blank"),
            Self::ControlInLabel => {
                formatter.write_str("a source-class label holds a control character")
            }
            Self::NoMatcher => {
                formatter.write_str("a source-class rule names no source_kind, host or path_prefix")
            }
            Self::EmptyMatcher => formatter
                .write_str("a source-class rule's source_kind, host or path_prefix is empty"),
            Self::RelativeHostPath => {
                formatter.write_str("a host rule's path_prefix must start with `/`")
            }
        }
    }
}

impl error::Error for TableError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Encoding
            | Self::Schema(_)
            | Self::UnknownClass(_)
            | Self::BlankLabel
            | Self::ControlInLabel
            | Self::NoMatcher
            | Self::EmptyMatcher
            | Self::RelativeHostPath => None,
        }
    }
}
