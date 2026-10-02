//! Trusted identity and review evidence injected by base-code CI, never catalog metadata.

use super::{
    Catalog, Diagnostic, EntryKind, Known, Ownership, Refusal, Registry, Resource, ReviewRole,
    SourceTree, Value, check::checked_snapshot, ownership::principal, placements::safe,
};
use crate::limits::Limits;
use maestro_kernel::artifact::Digest;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::collections::{BTreeMap, BTreeSet};
use std::{fmt, marker::PhantomData};

/// A checked revision and exact file pins retained from its single source read.
#[derive(Debug)]
pub struct OwnerSnapshot {
    /// Commit selected by the trusted caller, not by the catalog.
    revision: String,
    /// Decoded ownership from those same bytes.
    catalog: Catalog,
    /// Exact files, including inventoried assets and governance inputs.
    files: BTreeMap<String, Digest>,
}

impl OwnerSnapshot {
    /// Check an injected tree and retain its digests without reopening source files.
    ///
    /// # Errors
    /// Invalid revisions or catalog source checks refuse.
    pub fn check(
        revision: &str,
        tree: &dyn SourceTree,
        registry: &Registry,
        limits: &Limits,
        known: Known<'_>,
    ) -> Result<Self, Refusal> {
        if !revision_id(revision) {
            return Err(Refusal {
                diagnostics: vec![Diagnostic::new(
                    "",
                    "revision",
                    "supply a full lowercase Git commit ID from trusted CI",
                )],
            });
        }
        let (catalog, snapshot) = checked_snapshot(tree, registry, limits, known)?;
        let mut files = BTreeMap::new();
        for (directory, entry) in snapshot
            .directories
            .iter()
            .flat_map(|(directory, entries)| entries.iter().map(move |entry| (directory, entry)))
        {
            if entry.kind != EntryKind::File {
                continue;
            }
            let path = super::placements::join(directory, &entry.name);
            let bytes = snapshot
                .read(&path, limits.source_file_bytes)
                .map_err(|error| Refusal {
                    diagnostics: vec![Diagnostic::unreadable(&path, error.to_string())],
                })?;
            files.insert(path, Digest::of(&bytes));
        }
        Ok(Self {
            revision: revision.to_owned(),
            catalog,
            files,
        })
    }

    /// Exact file pins available for a trusted adapter to bind review evidence.
    #[must_use]
    pub fn files(&self) -> &BTreeMap<String, Digest> {
        &self.files
    }
}

/// A read-only lookup result, with team membership rather than a team approval actor.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalEvidence {
    /// User or organization/team handle in the descriptor's spelling.
    pub principal: String,
    /// Authenticated lookup found this principal.
    pub exists: bool,
    /// The lookup proved repository access.
    pub repository_access: bool,
    /// Team members; empty for users and refusing for unready teams.
    pub members: Vec<String>,
}

/// One exact file admitted by a review; absent digest binds a deletion.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalFile {
    /// Catalog-relative path.
    pub path: String,
    /// SHA-256 of the proposed file, or null for deletion.
    pub digest: Option<Digest>,
}

/// An authenticated review record supplied by trusted CI, not PR source code.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerApproval {
    /// User who submitted the review; teams cannot submit reviews.
    pub actor: String,
    /// Exact commit reviewed.
    pub head: String,
    /// Whether this is an effective approved review, not dismissed or revoked.
    pub approved: bool,
    /// Exact reviewed file digests, never a metadata admission flag.
    #[serde(deserialize_with = "objects")]
    pub files: Vec<ApprovalFile>,
}

/// Trusted, repository/revision-bound input. It is not a signature or runtime grant.
/// The adapter must obtain this outside PR code and protect its provenance.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerEvidence {
    /// `maestro-owner-evidence/1`.
    pub schema: String,
    /// Repository selected by the trusted caller.
    pub repository: String,
    /// Exact base commit used for ownership.
    pub base: String,
    /// Exact proposed commit used for digests and review.
    pub head: String,
    /// Authenticated principal/team lookup results.
    #[serde(deserialize_with = "objects")]
    pub principals: Vec<PrincipalEvidence>,
    /// Effective review records from the trusted adapter.
    #[serde(deserialize_with = "objects")]
    pub approvals: Vec<OwnerApproval>,
}

/// Verify all head principals and base-owner review over exact checked bytes.
/// No network, token, or editable admission field participates in this check.
///
/// # Errors
/// Missing/unverifiable identities, access or exact-head/digest approvals refuse.
pub fn check_owners(
    base: &OwnerSnapshot,
    head: &OwnerSnapshot,
    evidence: &OwnerEvidence,
    repository: &str,
) -> Result<(), String> {
    if evidence.schema != "maestro-owner-evidence/1"
        || evidence.repository != repository
        || repository.is_empty()
        || evidence.base != base.revision
        || evidence.head != head.revision
    {
        return Err(
            "owner evidence: obtain trusted evidence for this repository and exact base/head"
                .into(),
        );
    }
    root(&head.catalog)?;
    let identities = identities(evidence)?;
    for snapshot in [base, head] {
        for area in snapshot
            .catalog
            .resources
            .iter()
            .filter(|resource| resource.fields.contains_key("owners"))
        {
            let ownership = snapshot
                .catalog
                .ownership(area)
                .ok_or_else(|| format!("{}: missing area ownership", area.path))?;
            for name in ownership.owners.iter().chain(&ownership.maintainers) {
                ready(name, &identities).map_err(|error| format!("{}: {error}", area.path))?;
            }
        }
    }
    let paths: BTreeSet<_> = base.files.keys().chain(head.files.keys()).collect();
    for path in paths {
        let admission = resource_at(&head.catalog, path)
            .is_some_and(|resource| resource.id.kind == "knowledge-source");
        if base.files.get(path) != head.files.get(path) || admission {
            review_path(base, head, evidence, &identities, path)?;
        }
    }
    Ok(())
}

/// Full SHA-1 or SHA-256 Git object spelling; mutable refs are never revisions.
fn revision_id(revision: &str) -> bool {
    (revision.len() == 40 || revision.len() == 64)
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// GitHub handles are case-insensitive and allow the optional user sigil.
fn normalized(name: &str) -> String {
    name.strip_prefix('@').unwrap_or(name).to_ascii_lowercase()
}

/// Index strict lookup records and reject ambiguous actors/file pins.
fn identities(evidence: &OwnerEvidence) -> Result<BTreeMap<String, &PrincipalEvidence>, String> {
    let mut identities = BTreeMap::new();
    for record in &evidence.principals {
        if !principal(&record.principal)
            || identities
                .insert(normalized(&record.principal), record)
                .is_some()
        {
            return Err(format!(
                "owner evidence: malformed or duplicate principal {:?}",
                record.principal
            ));
        }
        let mut members = BTreeSet::new();
        for member in &record.members {
            if member.contains('/') || !members.insert(normalized(member)) {
                return Err(format!(
                    "owner evidence: malformed or duplicate team member {member:?}"
                ));
            }
        }
        if !record.principal.contains('/') && !record.members.is_empty() {
            return Err(format!(
                "owner evidence: user {} cannot assert team membership",
                record.principal
            ));
        }
    }
    for review in &evidence.approvals {
        if review.actor.contains('/') || !revision_id(&review.head) {
            return Err("owner evidence: review requires a user actor and full commit ID".into());
        }
        ready(&review.actor, &identities)
            .map_err(|error| format!("owner evidence review actor: {error}"))?;
        let mut paths = BTreeSet::new();
        for file in &review.files {
            if file.path.is_empty() || !safe(&file.path, false) || !paths.insert(&file.path) {
                return Err(format!(
                    "owner evidence: unsafe or duplicate review path {:?}",
                    file.path
                ));
            }
        }
    }
    Ok(identities)
}

/// Fail closed on absent lookups, unavailable access or unready team membership.
fn ready<'a>(
    name: &str,
    identities: &BTreeMap<String, &'a PrincipalEvidence>,
) -> Result<&'a PrincipalEvidence, String> {
    let record = identities.get(&normalized(name)).ok_or_else(|| {
        format!("{name}: missing trusted identity lookup; obtain read-only repository evidence")
    })?;
    if !record.exists || !record.repository_access {
        return Err(format!(
            "{name}: unknown principal or missing repository access; verify trusted lookup/access"
        ));
    }
    if name.contains('/') {
        if record.members.is_empty() {
            return Err(format!(
                "{name}: team is not ready; obtain verified membership"
            ));
        }
        for member in &record.members {
            ready(member, identities)?;
        }
    }
    Ok(record)
}

/// Find the canonical resource claiming a source/sidecar/asset path.
// ponytail: linear lookup per file; index claimed paths if large CI catalogs need it.
fn resource_at<'a>(catalog: &'a Catalog, path: &str) -> Option<&'a Resource> {
    catalog.resources.iter().find(|resource| {
        resource
            .files
            .iter()
            .chain(&resource.data)
            .any(|file| file == path)
    })
}

/// Root ownership, not a group or resource-local owner registry.
fn root(catalog: &Catalog) -> Result<Ownership<'_>, String> {
    catalog
        .resources
        .iter()
        .find(|resource| resource.path == "package.toml")
        .and_then(|resource| catalog.ownership(resource))
        .ok_or_else(|| {
            "package.toml: trusted ownership checking requires a checked base root descriptor"
                .into()
        })
}

/// Resolve head placement to its base-revision area, with new areas authorized centrally.
fn base_area<'a>(
    base: &'a Catalog,
    catalog: &Catalog,
    resource: &Resource,
) -> Result<Ownership<'a>, String> {
    let area = catalog
        .ownership(resource)
        .ok_or_else(|| format!("{}: missing area ownership", resource.path))?;
    base.resources
        .iter()
        .find(|candidate| candidate.id == area.descriptor.id)
        .and_then(|candidate| base.ownership(candidate))
        .map_or_else(|| root(base), Ok)
}

/// Verify a changed/admitted file against its stable area identity in the base.
fn review_path(
    base: &OwnerSnapshot,
    head: &OwnerSnapshot,
    evidence: &OwnerEvidence,
    identities: &BTreeMap<String, &PrincipalEvidence>,
    path: &str,
) -> Result<(), String> {
    let resource = resource_at(&head.catalog, path).or_else(|| resource_at(&base.catalog, path));
    let catalog = if resource_at(&head.catalog, path).is_some() {
        &head.catalog
    } else {
        &base.catalog
    };
    let area = resource.map_or_else(
        || root(&base.catalog),
        |resource| base_area(&base.catalog, catalog, resource),
    )?;
    let protected = area.protected_paths();
    let new_area = resource
        .and_then(|resource| catalog.ownership(resource))
        .is_some_and(|owner| owner.descriptor.id != area.descriptor.id);
    let role = if new_area {
        ReviewRole::OwnersOnly
    } else {
        area.review_rules(&protected)
            .iter()
            .rev()
            .find(|rule| rule.path.covers(path))
            .map_or(ReviewRole::OwnersOnly, |rule| rule.role)
    };
    let mut reviewers = area.owners.clone();
    if role == ReviewRole::Content {
        reviewers.extend(&area.maintainers);
    }
    require_review(&reviewers, path, head, evidence, identities)?;
    for catalog in [&base.catalog, &head.catalog] {
        if let Some(exception) =
            resource_at(catalog, path).filter(|resource| resource.id.kind == "standard-exception")
        {
            exception_reviews(
                &base.catalog,
                catalog,
                exception,
                path,
                (head, evidence, identities),
            )?;
        }
    }
    Ok(())
}

/// Independent standard and central approvals; an evidence reference is not a review.
fn exception_reviews(
    base: &Catalog,
    catalog: &Catalog,
    exception: &Resource,
    path: &str,
    (head, evidence, identities): (
        &OwnerSnapshot,
        &OwnerEvidence,
        &BTreeMap<String, &PrincipalEvidence>,
    ),
) -> Result<(), String> {
    let rule = exception
        .fields
        .get("rule")
        .and_then(Value::text)
        .ok_or("missing exception rule")?;
    let standard = catalog
        .resources
        .iter()
        .find(|resource| {
            resource.id.kind == "standard"
                && resource
                    .fields
                    .get("rules")
                    .and_then(Value::texts)
                    .unwrap_or_default()
                    .contains(&rule)
        })
        .ok_or_else(|| format!("{path}: missing owning standard"))?;
    let standard_area = base_area(base, catalog, standard)?;
    require_review(&standard_area.owners, path, head, evidence, identities)?;
    require_review(&root(base)?.owners, path, head, evidence, identities)
}

/// Match an effective review to exact head, exact bytes/absence and verified base principals.
fn require_review(
    reviewers: &[&str],
    path: &str,
    head: &OwnerSnapshot,
    evidence: &OwnerEvidence,
    identities: &BTreeMap<String, &PrincipalEvidence>,
) -> Result<(), String> {
    for review in &evidence.approvals {
        if !review.approved || review.head != head.revision {
            continue;
        }
        if !review
            .files
            .iter()
            .any(|file| file.path == path && file.digest.as_ref() == head.files.get(path))
        {
            continue;
        }
        for name in reviewers {
            let identity = ready(name, identities)?;
            let authorized = if name.contains('/') {
                identity
                    .members
                    .iter()
                    .any(|member| normalized(member) == normalized(&review.actor))
            } else {
                normalized(name) == normalized(&review.actor)
            };
            if authorized {
                return Ok(());
            }
        }
    }
    Err(format!(
        "{path}: missing effective base-principal approval of exact head/digest; \
         obtain a fresh review from {reviewers:?}"
    ))
}

/// A map-only serde adapter: derived structs otherwise accept positional arrays.
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Forward only map access to the type's strict derived field visitor.
        struct Map<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for Map<T> {
            type Value = Object<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an evidence object, not a positional array")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(de::value::MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(Map(PhantomData))
    }
}

/// Nested evidence records must also be objects; unknown/duplicate keys stay strict.
fn objects<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<Object<T>>::deserialize(deserializer)
        .map(|objects| objects.into_iter().map(|object| object.0).collect())
}

impl OwnerEvidence {
    /// Parse strict JSON objects; this data is trusted only through its external adapter.
    ///
    /// # Errors
    /// Arrays in place of records, unknown/duplicate keys and invalid digest types refuse.
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice::<Object<Self>>(bytes).map(|object| object.0)
    }
}
