//! One pure admission path: current caller, URL/network/robots, content, cache.
use super::{
    decisions::{Action, Decisions, Promotion},
    identity::{FetchIdentity, within},
    shape,
    source::{Selector, Source},
};
use crate::{ports::CheckedPolicy, refusal::Refusal};
use std::fmt::Debug;

/// Discovery/resumption never changes denial precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// Declared seed.
    Seed,
    /// Manually followed redirect.
    Redirect,
    /// Child resource from a page or browser.
    Subresource,
    /// Pending work under current authority.
    Resume,
    /// A retry with fresh checks, never an exception.
    Retry,
}
/// Explicit object metadata for conjunctive policy selectors; absent is unknown.
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemAttributes<'a> {
    /// Declared source object ID.
    pub object_id: Option<&'a str>,
    /// Observed version.
    pub version: Option<&'a str>,
    /// Observed channel.
    pub channel: Option<&'a str>,
    /// Observed media type.
    pub media_type: Option<&'a str>,
}
/// A candidate only, never permission to dispatch effects.
#[derive(Debug)]
pub struct Request<'a> {
    /// Declared source ID.
    pub source_id: &'a str,
    /// Untrusted candidate URL, parsed only after current caller checks.
    pub url: &'a str,
    /// Provenance supplied to current authority checks, not a bypass switch.
    pub kind: RequestKind,
    /// Explicit selector dimensions; missing values never defeat a denial.
    pub attributes: ItemAttributes<'a>,
    /// Explicit caller cache-refresh request; honored only after all controls.
    pub cache_bypass: bool,
    /// Current UTC time supplied by the host, not the immutable policy.
    pub now: &'a str,
}
/// Replaceable read-only current controls; implementations cannot dispatch fetches.
pub trait AdmissionControls: Debug {
    /// Check fresh caller/source authority and required bindings before URL work.
    ///
    /// # Errors
    /// Missing, revoked, expired or unbound authority refuses.
    fn caller(&self, source: &Source, request: &Request<'_>) -> Result<(), Refusal>;
    /// Check the current network policy; N08 owns address classification/pinning.
    ///
    /// # Errors
    /// Unknown, unavailable or denied network evidence refuses.
    fn network(&self, identity: &FetchIdentity) -> Result<(), Refusal>;
    /// Check robots under the same destination; N10 owns RFC 9309 and overrides.
    ///
    /// # Errors
    /// Missing/unreadable/denied robots evidence refuses.
    fn robots(&self, identity: &FetchIdentity) -> Result<(), Refusal>;
}
/// Content disposition is separate from permission to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Eligible for downstream knowledge gates, not publication approval.
    Knowledge,
    /// May capture, but remains excluded from knowledge.
    Excluded,
    /// May capture only as an asset.
    AssetOnly,
}
/// A pure checked result; transport must still admit/pin each actual connection.
#[derive(Debug)]
pub struct FetchAdmission {
    /// Exact source/rule-bound fetch identity.
    identity: FetchIdentity,
    /// Content disposition after independent promotions.
    disposition: Disposition,
    /// Explicit bypass honored last; never bypasses admission or transport checks.
    cache_bypass: bool,
}
impl FetchAdmission {
    /// Exact destination to hand to the admitted transport.
    #[must_use]
    pub fn identity(&self) -> &FetchIdentity {
        &self.identity
    }
    /// Separate downstream content eligibility.
    #[must_use]
    pub fn disposition(&self) -> Disposition {
        self.disposition
    }
    /// Whether this admitted request explicitly bypasses cache reuse.
    #[must_use]
    pub fn cache_bypass(&self) -> bool {
        self.cache_bypass
    }
}

/// Every seed/hop/subresource/resume/retry uses the same denial-first path.
///
/// # Errors
/// Any current caller, URL/selection, network, robots or fetch denial refuses.
/// Unknown selector semantics hold; promotion/cache bypass cannot override them.
pub fn admit(
    policy: &CheckedPolicy,
    request: &Request<'_>,
    controls: &dyn AdmissionControls,
) -> Result<FetchAdmission, Refusal> {
    let source = policy
        .policy
        .sources
        .iter()
        .find(|source| source.id == request.source_id)
        .ok_or(Refusal::Access)?;
    controls.caller(source, request)?;
    if !shape::valid_time(request.now) {
        return Err(Refusal::Invalid);
    }
    let identity = FetchIdentity::parse(source, request.url)?;
    check_selection(source, &identity, request.attributes)?;
    let mut disposition = Disposition::Knowledge;
    for registry in &policy.decisions {
        for entry in &registry.entries {
            if time_key(&entry.effective_at) > time_key(request.now) {
                continue;
            }
            if !matches_selector(&entry.selector, &identity, request.attributes)? {
                continue;
            }
            // Expired exclusions hold; expiry is never automatic re-admission.
            disposition = match entry.action {
                Action::DenyFetch => return Err(Refusal::Access),
                Action::ExcludeFromKnowledge => Disposition::Excluded,
                Action::AssetOnly => Disposition::AssetOnly,
            };
        }
    }
    controls.network(&identity)?;
    controls.robots(&identity)?;
    for promotion in policy.promotions.get(&source.id).into_iter().flatten() {
        if promotion_active(promotion, request.now)
            && matches_selector(&promotion.selector, &identity, request.attributes).unwrap_or(false)
        {
            disposition = Disposition::Knowledge;
        }
    }
    Ok(FetchAdmission {
        identity,
        disposition,
        cache_bypass: request.cache_bypass,
    })
}

/// Fresh policy-only target checks also apply to migration replacement URLs.
pub(super) fn check_target(
    source: &Source,
    identity: &FetchIdentity,
    registries: &[Decisions],
) -> Result<(), Refusal> {
    check_selection(source, identity, ItemAttributes::default())?;
    for registry in registries {
        for entry in &registry.entries {
            if entry.action == Action::DenyFetch
                && matches_selector(&entry.selector, identity, ItemAttributes::default())?
            {
                return Err(Refusal::Access);
            }
        }
    }
    Ok(())
}
/// Robots are derived only from an admitted origin, not content-path selectors.
pub(super) fn check_robots(
    source: &Source,
    identity: &FetchIdentity,
    registries: &[Decisions],
) -> Result<(), Refusal> {
    if !source.selectors.iter().any(|selector| {
        selector.source_id == source.id
            && selector
                .origin
                .as_deref()
                .is_none_or(|origin| origin == identity.origin_id())
    }) {
        return Err(Refusal::Access);
    }
    for entry in registries.iter().flat_map(|registry| &registry.entries) {
        if entry.action == Action::DenyFetch
            && matches_selector(&entry.selector, identity, ItemAttributes::default())?
        {
            return Err(Refusal::Access);
        }
    }
    Ok(())
}

/// Disjunctive allowed selectors, each internally conjunctive; unknown holds.
fn check_selection(
    source: &Source,
    identity: &FetchIdentity,
    attributes: ItemAttributes<'_>,
) -> Result<(), Refusal> {
    if source
        .selectors
        .iter()
        .any(|selector| matches_selector(selector, identity, attributes).unwrap_or(false))
    {
        Ok(())
    } else {
        Err(Refusal::Access)
    }
}
/// Exact conjunction of present fields, disjunction within each value list.
fn matches_selector(
    selector: &Selector,
    identity: &FetchIdentity,
    attributes: ItemAttributes<'_>,
) -> Result<bool, Refusal> {
    if selector.source_id != identity.source_id()
        || selector
            .origin
            .as_deref()
            .is_some_and(|origin| origin != identity.origin_id())
        || selector
            .path_prefix
            .as_deref()
            .is_some_and(|prefix| !within(identity.url().path(), prefix))
    {
        return Ok(false);
    }
    let dimensions = [
        (&selector.object_ids, attributes.object_id),
        (&selector.versions, attributes.version),
        (&selector.channels, attributes.channel),
        (&selector.media_types, attributes.media_type),
    ];
    for (values, actual) in dimensions {
        if !values.is_empty()
            && actual.is_some_and(|actual| !values.iter().any(|value| value == actual))
        {
            return Ok(false);
        }
    }
    if dimensions
        .iter()
        .any(|(values, actual)| !values.is_empty() && actual.is_none())
    {
        return Err(Refusal::Access);
    }
    Ok(true)
}
/// Promotions expire closed, unlike exclusions which retain their hold.
fn promotion_active(promotion: &Promotion, now: &str) -> bool {
    time_key(&promotion.effective_at) <= time_key(now)
        && promotion
            .expires_at
            .as_deref()
            .is_none_or(|expires| time_key(now) < time_key(expires))
}
/// Strict UTC shape permits lexical tuple ordering, with fractional zero equal.
pub(crate) fn time_key(text: &str) -> (&str, &str) {
    let text = text.trim_end_matches('Z');
    let (seconds, fraction) = text.split_once('.').unwrap_or((text, ""));
    (seconds, fraction.trim_end_matches('0'))
}

impl CheckedPolicy {
    /// Derive only `/robots.txt`, without query/fragment, for a selected origin.
    /// This does not admit robots as content or create authority to dispatch.
    /// Each hop still needs fresh N05 authority and N08 destination admission.
    ///
    /// # Errors
    /// Unknown source/origin, another path, or a current fetch denial refuses.
    pub fn admit_robots(&self, source_id: &str, url: &str) -> Result<FetchIdentity, Refusal> {
        let source = self
            .policy
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or(Refusal::Access)?;
        let identity = FetchIdentity::parse_operation(source, url, true)?;
        check_robots(source, &identity, &self.decisions)?;
        Ok(identity)
    }
}
