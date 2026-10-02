//! Phase 1 area versions: exact dependency pins and bounded runtime requirements.

use super::types::{Problems, Resource, ResourceId, Value};
use semver::{Comparator, Op, Version, VersionReq};
use std::collections::BTreeMap;

/// The shared exact `SemVer` parser for package versions and dependency pins.
pub(super) fn exact_version(text: &str) -> Result<Version, semver::Error> {
    Version::parse(text)
}

/// Exact Phase 1 pins have an explicit equals sign and no prerelease/build suffix.
fn pin(text: &str) -> Option<Version> {
    let version = exact_version(text.strip_prefix('=')?).ok()?;
    (version.pre.is_empty() && version.build.is_empty()).then_some(version)
}

/// A comparator must name all three stable version components.
fn stable(comparator: &Comparator) -> bool {
    comparator.minor.is_some() && comparator.patch.is_some() && comparator.pre.is_empty()
}

/// Only the approved exact or ordered lower-inclusive/upper-exclusive interval.
fn runtime_requirement(text: &str) -> Option<VersionReq> {
    if text.contains('+') {
        return None;
    }
    let requirement = VersionReq::parse(text).ok()?;
    let supported = match requirement.comparators.as_slice() {
        [exact] => exact.op == Op::Exact && stable(exact),
        [lower, upper] => {
            lower.op == Op::GreaterEq && upper.op == Op::Less && stable(lower) && stable(upper)
        }
        _ => false,
    };
    supported.then_some(requirement)
}

/// All workspace crates use `version.workspace = true`, so the catalog's build
/// version is also the current Maestro runtime version; never a source-supplied value.
pub(super) fn check_resource(resource: &Resource, problems: &mut Problems) {
    if let Some(text) = resource.fields.get("runtime").and_then(Value::text) {
        if let Some(requirement) = runtime_requirement(text) {
            let current = env!("CARGO_PKG_VERSION");
            if !exact_version(current).is_ok_and(|version| requirement.matches(&version)) {
                problems.push((
                    "runtime".to_owned(),
                    format!(
                        "{}: incompatible runtime {current}; requires {text}",
                        resource.id
                    ),
                ));
            }
        } else {
            problems.push((
                "runtime".to_owned(),
                format!(
                    "{}: runtime requirement {text} unsupported in Phase 1; \
                     use =A.B.C or >=A.B.C, <D.E.F",
                    resource.id
                ),
            ));
        }
    }
    let Some(Value::Table(pins)) = resource.fields.get("dependency_pins") else {
        return;
    };
    for (target, value) in pins {
        let key = format!("dependency_pins.{target}");
        let required = ResourceId::parse(target).is_some_and(|id| {
            ["package", "language", "standard"].contains(&id.kind.as_str())
                && resource.metadata.requires.contains(&id)
        });
        if !required {
            problems.push((
                key.clone(),
                format!("{target} must name a required area in metadata.requires"),
            ));
        }
        if value.text().and_then(pin).is_none() {
            problems.push((
                key,
                "must be an exact stable =MAJOR.MINOR.PATCH pin; \
                 dependency intervals belong to Phase 2"
                    .to_owned(),
            ));
        }
    }
}

/// Snapshot equality enforces one version per target, regardless of declaring order.
/// Dangling targets are already refused by the shared reference checker.
pub(super) fn check_catalog(
    resource: &Resource,
    catalog: &BTreeMap<ResourceId, &Resource>,
    problems: &mut Problems,
) {
    let Some(Value::Table(pins)) = resource.fields.get("dependency_pins") else {
        return;
    };
    for (target, value) in pins {
        let Some(target_id) = ResourceId::parse(target) else {
            continue;
        };
        let Some(required) = catalog.get(&target_id) else {
            continue;
        };
        let Some(pinned) = value.text().and_then(pin) else {
            continue;
        };
        let actual = required
            .fields
            .get("version")
            .and_then(Value::text)
            .and_then(|text| exact_version(text).ok());
        if actual.as_ref() != Some(&pinned) {
            problems.push((
                format!("dependency_pins.{target}"),
                format!(
                    "conflicting exact pin for {target}: requires {pinned}, snapshot has {}",
                    required
                        .fields
                        .get("version")
                        .and_then(Value::text)
                        .unwrap_or("no version")
                ),
            ));
        }
    }
}
