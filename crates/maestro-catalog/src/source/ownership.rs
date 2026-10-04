//! Offline area principals and ordered review delegation, not approval evidence.

use super::{
    descriptor::{KindDescriptor, Layout, Scope},
    placements::{directories, fits, join, safe},
    types::{Catalog, Problems, Resource, Value},
};
use crate::limits::Limits;
use std::{collections::BTreeSet, iter::once};

/// A known generated file, counted in the snapshot and validated after area loading.
pub(super) struct GeneratedFile {
    /// Exact catalog-relative output path, never a directory exemption.
    pub(super) path: &'static str,
    /// Validator over the checked area's canonical records.
    pub(super) validate: fn(&Catalog, &str) -> Result<(), String>,
}

/// The generated ownership entry: discovery and validation use this same record.
pub(super) const GENERATED_CODEOWNERS: GeneratedFile = GeneratedFile {
    path: ".github/CODEOWNERS",
    validate: Catalog::check_codeowners,
};

/// The sole generated CODEOWNERS path, relative to the catalog.
pub const CODEOWNERS_PATH: &str = GENERATED_CODEOWNERS.path;

/// The generated public JSON navigation, relative to the catalog.
pub const INDEX_PATH: &str = "marketplace/index.json";

/// The generated public type navigation, relative to the catalog.
pub const BY_TYPE_PATH: &str = "docs/catalog/by-type.md";

/// Exact generated outputs, never directory exemptions or source resources.
pub(super) const GENERATED_PATHS: [&str; 3] = [CODEOWNERS_PATH, INDEX_PATH, BY_TYPE_PATH];

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
    /// A missing or ambiguous area, or an inconsistent ID namespace, returns `None`.
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
        let common = !Scope::AREA_ROOTS
            .iter()
            .any(|scope| scope.prefix().split('/').next() == resource.path.split('/').next());
        let mut candidates = self
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
                (parent.is_empty() && common)
                    || resource
                        .path
                        .strip_prefix(parent)
                        .is_some_and(|tail| tail.starts_with('/'))
            });
        let descriptor = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
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
pub(super) fn principal(text: &str) -> bool {
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
    /// The canonical exception/governance protections shared by rendering and CI.
    pub(super) fn protected_paths(&self) -> Vec<ReviewPath> {
        if self.descriptor.path.contains('/') {
            let parent = self
                .descriptor
                .path
                .rsplit_once('/')
                .map_or("", |(parent, _)| parent);
            vec![ReviewPath::Tree(join(parent, Scope::EXCEPTIONS))]
        } else {
            Scope::SUPPORT_ROOTS
                .into_iter()
                .chain(Scope::ROOT_GOVERNANCE)
                .map(|path| ReviewPath::Tree(path.to_owned()))
                .collect()
        }
    }

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

impl Catalog {
    /// Render anchored CODEOWNERS from checked area records, content first.
    /// Root governance and descriptor/exception rules are owners-only and win last.
    /// This is review routing, not identity verification or independent quorums.
    ///
    /// # Errors
    /// Missing/ambiguous area ownership, unsafe literal placement or oversized output.
    pub fn codeowners(&self) -> Result<String, String> {
        let root = self
            .resources
            .iter()
            .find(|resource| {
                resource.id.namespace.is_none()
                    && resource.fields.contains_key("owners")
                    && !resource.path.contains('/')
            })
            .and_then(|resource| self.ownership(resource))
            .ok_or_else(|| "CODEOWNERS requires unambiguous root area ownership".to_owned())?;
        let mut areas: Vec<_> = self
            .resources
            .iter()
            .filter(|resource| {
                resource.id.namespace.is_none() && resource.fields.contains_key("owners")
            })
            .collect();
        areas.sort_by_key(|resource| (resource.path != root.descriptor.path, &resource.path));
        let mut records = Vec::new();
        for area in areas {
            let ownership = self
                .ownership(area)
                .ok_or_else(|| format!("missing or ambiguous ownership for {}", area.path))?;
            let protected = ownership.protected_paths();
            let rules = ownership.review_rules(&protected);
            ownership.check_review_rules(&rules, &protected)?;
            records.push((ownership, rules));
        }
        let mut text = "# Generated from area descriptors; do not edit.\n\
            # Review routing only; no identity or approval evidence.\n"
            .to_owned();
        for role in [ReviewRole::Content, ReviewRole::OwnersOnly] {
            let rules = records
                .iter()
                .flat_map(|(ownership, rules)| rules.iter().map(move |rule| (ownership, rule)));
            for (ownership, rule) in rules.filter(|(_, rule)| rule.role == role) {
                text.push_str(&render_rule(
                    &rule.path,
                    &reviewers(ownership, rule, &root.owners),
                )?);
            }
        }
        within(&text, Limits::PRODUCTION.source_file_bytes)?;
        Ok(text)
    }

    /// Compare committed CODEOWNERS byte for byte without writing anything.
    ///
    /// # Errors
    /// The first missing, extra, stale, edited or reordered rule, naming both sides.
    pub fn check_codeowners(&self, actual: &str) -> Result<(), String> {
        let expected = self.codeowners()?;
        let expected: Vec<_> = expected.split_inclusive('\n').collect();
        let actual: Vec<_> = actual.split_inclusive('\n').collect();
        for index in 0..expected.len().max(actual.len()) {
            if expected.get(index) != actual.get(index) {
                return Err(format!(
                    "CODEOWNERS rule drift at line {}: expected {:?}; found {:?}",
                    index + 1,
                    expected.get(index).unwrap_or(&"<no rule>"),
                    actual.get(index).unwrap_or(&"<missing rule>")
                ));
            }
        }
        Ok(())
    }
}

/// Delegates apply only to content; standard exceptions also name central owners.
fn reviewers<'a>(ownership: &Ownership<'a>, rule: &ReviewRule, root: &[&'a str]) -> Vec<&'a str> {
    let mut reviewers = ownership.owners.clone();
    if rule.role == ReviewRole::Content {
        reviewers.extend(&ownership.maintainers);
    } else if ownership.descriptor.id.kind == "standard" && matches!(rule.path, ReviewPath::Tree(_))
    {
        reviewers.extend(root);
    }
    reviewers
}

/// Literal anchored patterns and normalized, case-insensitively deduplicated handles.
fn render_rule(path: &ReviewPath, reviewers: &[&str]) -> Result<String, String> {
    let (literal, suffix) = match path {
        ReviewPath::Exact(path) => (path.as_str(), ""),
        ReviewPath::Tree(path) if path.is_empty() => ("", "*"),
        ReviewPath::Tree(path) => (path.as_str(), "/"),
    };
    if !literal
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"-_./".contains(&byte))
    {
        return Err(format!("unsafe CODEOWNERS rule path {literal:?}"));
    }
    let mut seen = BTreeSet::new();
    let mut handles = Vec::new();
    for reviewer in reviewers {
        let handle = format!("@{}", reviewer.strip_prefix('@').unwrap_or(reviewer));
        if seen.insert(handle.to_ascii_lowercase()) {
            handles.push(handle);
        }
    }
    Ok(format!("/{literal}{suffix} {}\n", handles.join(" ")))
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

/// Refuse rendered text that the committed-file check would refuse: at most `limit` bytes.
pub(super) fn within(text: &str, limit: u64) -> Result<(), String> {
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > limit {
        return Err(format!("{CODEOWNERS_PATH}: larger than {limit} bytes"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::locate;
    use crate::source::builtin;

    #[test]
    fn ownership_location_rewrites_only_legacy_ownership_unknown_or_ambiguous_errors() {
        let registry = builtin().unwrap();
        let descriptor = &registry.kind("agent").unwrap().descriptor;
        let mut problems = vec![
            ("owners".to_owned(), "unknown key".to_owned()),
            ("owners".to_owned(), "must be a list".to_owned()),
            ("other".to_owned(), "unknown key".to_owned()),
        ];
        locate("core/agents/example.agent.md", descriptor, &mut problems);
        assert_eq!(
            problems,
            [
                (
                    "owners".to_owned(),
                    "unknown key; ownership is derived from core/package.toml".to_owned()
                ),
                ("owners".to_owned(), "must be a list".to_owned()),
                ("other".to_owned(), "unknown key".to_owned())
            ]
        );
    }
}
