//! Scope paths: a workspace, then optionally a collection, then optionally a
//! source, each named by the rule collection and source IDs follow, and what
//! a scope covers.

use std::{error, fmt, str::FromStr};

/// The kinds of the nodes a scope path names, in the order they nest.
const KINDS: [&str; 3] = ["workspace", "collection", "source"];

/// The scope of the workspace the kernel keeps its records in: S1 has one.
pub const WORKSPACE: &str = "workspace/default";

/// The most characters a name holds.
const LONGEST_NAME: usize = 64;

/// A node of the kernel's access tree: `workspace/<name>`, then optionally
/// `collection/<name>`, then optionally `source/<name>`, each name one that
/// [`check_name`] accepts. [`str::parse`] reads one from its path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scope(String);

impl Scope {
    /// Its path, such as `workspace/default/collection/ctm`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `scope` is this scope or lies below it, compared by whole
    /// segments: `…/collection/ct` covers `…/collection/ct/source/docs`, and
    /// never `…/collection/ctm`.
    #[must_use]
    pub fn covers(&self, scope: &Self) -> bool {
        scope
            .0
            .strip_prefix(&self.0)
            .is_some_and(|below| below.is_empty() || below.starts_with('/'))
    }
}

impl FromStr for Scope {
    type Err = InvalidScope;

    /// The scope `path` names.
    ///
    /// # Errors
    ///
    /// [`InvalidScope`], naming the path, when it is not a workspace, then
    /// optionally a collection, then optionally a source, or when one of its
    /// names breaks the rule of [`check_name`].
    fn from_str(path: &str) -> Result<Self, InvalidScope> {
        let refusal = |fault| InvalidScope {
            path: path.to_owned(),
            fault,
        };
        let mut segments = path.split('/');
        for kind in KINDS {
            let Some(found) = segments.next() else {
                break;
            };
            match segments.next() {
                Some(name) if found == kind => {
                    check_name(name).map_err(|invalid| refusal(Fault::Name(invalid)))?;
                }
                _ => return Err(refusal(Fault::Shape)),
            }
        }
        match segments.next() {
            Some(_) => Err(refusal(Fault::Shape)),
            None => Ok(Self(path.to_owned())),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The path of the scope of the collection `collection`, which every record
/// of the collection has, `workspace/default/collection/<id>`: the one place
/// a collection's scope is written, which the readers' conditions filter by.
#[must_use]
pub fn collection_path(collection: &str) -> String {
    format!("{WORKSPACE}/collection/{collection}")
}

/// The path of the scope of the source `source` of the collection
/// `collection`, which the documents and revisions of that source have:
/// `workspace/default/collection/<id>/source/<id>`.
#[must_use]
pub fn source_path(collection: &str, source: &str) -> String {
    format!("{}/source/{source}", collection_path(collection))
}

/// Checks that `name` may name a node of a scope path: 1 to 64 characters,
/// each a lowercase ASCII letter, a digit, `-`, `_` or `.`, the first a
/// letter or a digit. Collection and source IDs follow the same rule, so each
/// forms a segment of the path of the scope it names.
///
/// # Errors
///
/// [`InvalidName`], naming it, when it breaks the rule.
pub fn check_name(name: &str) -> Result<(), InvalidName> {
    let mut characters = name.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && characters.all(|next| {
            next.is_ascii_lowercase() || next.is_ascii_digit() || matches!(next, '-' | '_' | '.')
        })
        && name.len() <= LONGEST_NAME;
    if valid {
        Ok(())
    } else {
        Err(InvalidName(name.to_owned()))
    }
}

/// A name that breaks the rule of [`check_name`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidName(String);

impl fmt::Display for InvalidName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` is not a scope name: a name is 1 to {LONGEST_NAME} characters, each a \
             lowercase ASCII letter, a digit, `-`, `_` or `.`, and starts with a letter or \
             a digit",
            self.0
        )
    }
}

impl error::Error for InvalidName {}

/// Why a text is not a scope path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidScope {
    /// The text refused.
    path: String,
    /// What is wrong with it.
    fault: Fault,
}

/// What is wrong with a text that is not a scope path.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Fault {
    /// Its kinds are not a workspace, then optionally a collection, then
    /// optionally a source, or a kind has no name.
    Shape,
    /// One of its names breaks the rule.
    Name(InvalidName),
}

impl fmt::Display for InvalidScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "`{}` is not a scope: ", self.path)?;
        match &self.fault {
            Fault::Shape => formatter.write_str(
                "a scope is `workspace/<name>`, then optionally `collection/<name>`, then \
                 optionally `source/<name>`",
            ),
            Fault::Name(invalid) => fmt::Display::fmt(invalid, formatter),
        }
    }
}

impl error::Error for InvalidScope {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.fault {
            Fault::Shape => None,
            Fault::Name(invalid) => Some(invalid),
        }
    }
}
