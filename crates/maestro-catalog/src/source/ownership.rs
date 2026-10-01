//! Offline area principals and ordered review delegation, not approval evidence.

use super::{
    descriptor::{KindDescriptor, Layout, Scope},
    placements::{directories, fits, join, safe},
    types::{Catalog, Problems, Resource, Value},
};
use std::{collections::BTreeSet, iter::once};

/// The sole area principal policy: owners are required, content delegates optional.
const OWNERSHIP_FIELDS: [(&str, Option<bool>); 3] = [
    ("owner", None),
    ("owners", Some(true)),
    ("maintainers", Some(false)),
];

/// A borrowed area's review principals; never an editable resource owner list.
#[derive(Debug, PartialEq, Eq)]
pub struct Ownership<'a> {
    /// The authoritative area descriptor.
    pub descriptor: &'a Resource,
    /// Accountable users or teams.
    pub owners: Vec<&'a str>,
    /// Content reviewers, empty when absent.
    pub maintainers: Vec<&'a str>,
}

impl Catalog {
    /// Derives principals from the resource's area, including common-owned support.
    /// A catalog returned by the checker always has an ownership record.
    /// An ID namespace inconsistent with its physical area returns `None`.
    /// Declarations and offline validation confer no GitHub approval or runtime grant.
    #[must_use]
    pub fn ownership(&self, resource: &Resource) -> Option<Ownership<'_>> {
        let namespace = resource.id.namespace.as_deref().unwrap_or_else(|| {
            if resource.fields.contains_key("owners") {
                &resource.id.name
            } else {
                "common"
            }
        });
        let descriptor = self
            .resources
            .iter()
            .filter(|candidate| {
                candidate.id.namespace.is_none() && candidate.fields.contains_key("owners")
            })
            .filter(|candidate| {
                let parent = candidate
                    .path
                    .rsplit_once('/')
                    .map_or("", |(parent, _)| parent);
                parent.is_empty()
                    || resource
                        .path
                        .strip_prefix(parent)
                        .is_some_and(|tail| tail.starts_with('/'))
            })
            .max_by_key(|candidate| candidate.path.len())?;
        if descriptor.id.name != namespace {
            return None;
        }
        Some(Ownership {
            descriptor,
            owners: descriptor.fields.get("owners")?.texts()?,
            maintainers: descriptor
                .fields
                .get("maintainers")
                .and_then(Value::texts)
                .unwrap_or_default(),
        })
    }
}

/// Checks principals with GitHub's ASCII username grammar and team slug grammar.
fn principals(resource: &Resource, problems: &mut Problems) {
    for (field, required) in OWNERSHIP_FIELDS {
        let Some(required) = required else {
            continue;
        };
        let entries = match resource.fields.get(field) {
            Some(value) => {
                let Some(entries) = value.texts() else {
                    problems.push((field.to_owned(), "must be a list of strings".to_owned()));
                    continue;
                };
                entries
            }
            None => Vec::new(),
        };
        if required && entries.is_empty() {
            problems.push((field.to_owned(), "must be a nonempty list".to_owned()));
        }
        let mut seen = BTreeSet::new();
        for entry in entries {
            if !principal(entry) {
                problems.push((
                    field.to_owned(),
                    format!("malformed GitHub principal {entry:?}"),
                ));
            }
            if !seen.insert(
                entry
                    .strip_prefix('@')
                    .unwrap_or(entry)
                    .to_ascii_lowercase(),
            ) {
                problems.push((
                    field.to_owned(),
                    format!("duplicate GitHub principal {entry:?}"),
                ));
            }
        }
    }
}

/// Usernames may have single interior hyphens; team slugs may also have underscores.
fn slug(text: &str, team: bool) -> bool {
    let limit = if team { 100 } else { 39 };
    text.len() <= limit
        && text
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && text
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || (team && byte == b'_'))
        && (team || !text.contains("--"))
}

/// No network lookup: syntactic validity alone never grants review authority.
fn principal(text: &str) -> bool {
    if let Some(handle) = text.strip_prefix('@') {
        if let Some((org, team)) = handle.split_once('/') {
            return slug(org, false) && slug(team, true);
        }
        return slug(handle, false);
    }
    slug(text, false)
}

/// Areas require principals; resource-local fields cannot delegate ownership.
pub(super) fn local(resource: &Resource, descriptor: &KindDescriptor, problems: &mut Problems) {
    if matches!(descriptor.layout, Layout::Area { .. }) {
        principals(resource, problems);
    } else {
        for (field, _) in OWNERSHIP_FIELDS {
            if resource.fields.contains_key(field) {
                problems.push((field.to_owned(), "unknown key".to_owned()));
            }
        }
    }
}

/// Point refused legacy declarations to the authoritative area descriptor.
pub(super) fn locate(path: &str, descriptor: &KindDescriptor, problems: &mut Problems) {
    let area = area_path(path, descriptor);
    for (key, message) in problems {
        if OWNERSHIP_FIELDS
            .iter()
            .any(|(field, _)| Some(*field) == key.rsplit('.').next())
            && (message == "unknown key" || message.starts_with("ambiguous;"))
        {
            *message = format!("{message}; ownership is derived from {area}");
        }
    }
}

/// The registered placement's exact area descriptor, never a group ancestor.
pub(super) fn area_path(path: &str, descriptor: &KindDescriptor) -> String {
    if matches!(descriptor.layout, Layout::Area { .. }) {
        return path.to_owned();
    }
    let boundary = descriptor
        .scopes
        .iter()
        .zip(directories(descriptor))
        .filter_map(|(scope, directory)| {
            let count = directory.split('/').filter(|part| !part.is_empty()).count();
            let boundary = path.split('/').take(count).collect::<Vec<_>>().join("/");
            if !fits(&directory, &boundary) {
                return None;
            }
            let area = match scope {
                Scope::Common | Scope::Root => String::new(),
                _ => path
                    .split('/')
                    .take(scope.prefix().split('/').count())
                    .collect::<Vec<_>>()
                    .join("/"),
            };
            Some((count, area))
        })
        .max_by_key(|(count, _)| *count)
        .map(|(_, area)| area)
        .unwrap_or_default();
    join(&boundary, "package.toml")
}

/// Literal anchored review placements; GitHub pattern rendering belongs to C35.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewPath {
    /// One exact relative file path.
    Exact(String),
    /// A relative directory and all its descendants; empty means the catalog.
    Tree(String),
}

impl ReviewPath {
    /// Safe catalog-relative literal paths; only a tree may name the empty root.
    fn literal(&self) -> bool {
        match self {
            Self::Exact(path) => !path.is_empty() && safe(path, false),
            Self::Tree(path) => safe(path, false),
        }
    }

    /// Whether a literal path is covered, with directory boundaries preserved.
    #[must_use]
    pub fn covers(&self, path: &str) -> bool {
        match self {
            Self::Exact(file) => file == path,
            Self::Tree(prefix) => {
                prefix.is_empty()
                    || path
                        .strip_prefix(prefix)
                        .is_some_and(|tail| tail.starts_with('/'))
            }
        }
    }
}

/// Review routing roles, not CI authorization or independent quorums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRole {
    /// Owners plus delegated maintainers review ordinary content.
    Content,
    /// Only area owners review protected changes.
    OwnersOnly,
}

/// An ordered rule; the last covering rule wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRule {
    /// Literal anchored placement.
    pub path: ReviewPath,
    /// Review principals selected from the area's ownership record.
    pub role: ReviewRole,
}

impl Ownership<'_> {
    /// Content first, then the descriptor and caller-supplied exception/governance paths.
    #[must_use]
    pub fn review_rules(&self, protected: &[ReviewPath]) -> Vec<ReviewRule> {
        let parent = self
            .descriptor
            .path
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent);
        let mut rules = vec![ReviewRule {
            path: ReviewPath::Tree(parent.to_owned()),
            role: ReviewRole::Content,
        }];
        rules.extend(
            once(ReviewPath::Exact(self.descriptor.path.clone()))
                .chain(protected.iter().cloned())
                .map(|path| ReviewRule {
                    path,
                    role: ReviewRole::OwnersOnly,
                }),
        );
        rules
    }

    /// Checks last-match protection, including broad rules after the descriptor.
    /// Tree protections must remain protected for every descendant, not just one sample.
    ///
    /// # Errors
    /// A missing or content-delegated protection, which would permit self-delegation.
    pub fn check_review_rules(
        &self,
        rules: &[ReviewRule],
        protected: &[ReviewPath],
    ) -> Result<(), String> {
        let descriptor = ReviewPath::Exact(self.descriptor.path.clone());
        if let Some(path) = once(&descriptor)
            .chain(protected)
            .chain(rules.iter().map(|rule| &rule.path))
            .find(|path| !path.literal())
        {
            return Err(format!(
                "review path must be a literal catalog-relative placement: {path:?}"
            ));
        }
        for path in once(&descriptor).chain(protected) {
            let last = rules.iter().rposition(|rule| contains(&rule.path, path));
            let Some(last) = last.filter(|last| {
                rules
                    .get(*last)
                    .is_some_and(|rule| rule.role == ReviewRole::OwnersOnly)
            }) else {
                return Err(format!(
                    "owners-only protection missing or overridden for {path:?}"
                ));
            };
            if rules.iter().skip(last + 1).any(|rule| {
                contains(path, &rule.path)
                    && rules
                        .iter()
                        .rev()
                        .find(|later| contains(&later.path, &rule.path))
                        .is_some_and(|later| later.role == ReviewRole::Content)
            }) {
                return Err(format!(
                    "content rule overrides owners-only protection for {path:?}"
                ));
            }
        }
        Ok(())
    }
}

/// One literal rule fully covers another placement.
fn contains(outer: &ReviewPath, inner: &ReviewPath) -> bool {
    match inner {
        ReviewPath::Exact(path) => outer.covers(path),
        ReviewPath::Tree(path) => match outer {
            ReviewPath::Exact(_) => false,
            ReviewPath::Tree(prefix) => prefix == path || outer.covers(path),
        },
    }
}
