//! RFC 9309 rules over N07's canonical fetch identity, without network effects.
//!
//! Maestro parses 2xx; 404/410 mean absent rules. Every other status, unreadable,
//! invalid or oversized body denies. N09 admits redirects hop by hop. Cache TTL
//! is policy-bound and capped at 24 hours; failures remain denied until refreshed
//! by an admitted, paced request. No stale-on-error or automatic override exists.
use crate::{
    Ref, Refusal,
    policy::{identity::FetchIdentity, source::Robots},
};
use std::{fmt::Debug, mem::take, str::from_utf8};

/// Replaceable parser result: rules cannot dispatch or grant overrides.
pub trait RobotsRules: Debug + Send + Sync {
    /// Match a product token and canonical path plus meaningful query octets.
    fn allowed(&self, agent: &str, identity: &FetchIdentity) -> bool;
}
/// Separately authenticated current override decision, never manifest authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideDecision {
    /// `OA4b` receipt for the exact requested source/origin/ref, currently valid.
    Granted(Ref),
    /// No current authority, including revoked or missing grants.
    Denied,
    /// Previously granted authority has expired.
    Expired,
}
/// Exact read-only authority request; caller identity belongs to the adapter.
#[derive(Debug)]
pub struct OverrideRequest<'a> {
    /// Closed consumer operation: always `robots_override`.
    pub operation: &'static str,
    /// Exact source namespace, not inferred from a robots rule.
    pub source_id: &'a str,
    /// Exact canonical HTTPS origin, including nondefault port.
    pub origin: &'a str,
    /// Policy-pinned receipt; the adapter verifies owner/operator approval,
    /// source/origin binding, revocation and `OA4b`'s at-most-24-hour expiry.
    pub receipt: &'a Ref,
    /// Current host time, in the adapter's agreed monotonic clock domain.
    pub now_ms: u64,
}
/// Read-only consumer port; N05 binds owner-authenticated authority later.
pub trait RobotsOverride: Debug {
    /// Recheck current authority on every request, including resume/retry.
    fn decide(&self, request: &OverrideRequest<'_>) -> OverrideDecision;
}
/// The only supplied adapter: no live overrides are granted by N10.
#[derive(Debug)]
pub struct DenyOverrides;
impl RobotsOverride for DenyOverrides {
    fn decide(&self, _request: &OverrideRequest<'_>) -> OverrideDecision {
        OverrideDecision::Denied
    }
}
/// A parsed RFC group, retaining equal-agent groups for merging at evaluation.
#[derive(Debug, Default)]
struct Group {
    /// Case-insensitive product tokens, or the fallback wildcard.
    agents: Vec<String>,
    /// Nonempty path rules; empty Allow/Disallow never match.
    rules: Vec<Rule>,
    /// Even an empty directive ends the user-agent header section.
    has_directive: bool,
}
/// One normalized path rule; longest match wins, Allow wins equal lengths.
#[derive(Debug)]
struct Rule {
    /// Percent-normalized UTF-8 octets, with wildcard syntax preserved.
    pattern: String,
    /// Whether an equally specific match permits the request.
    allow: bool,
}
/// Bounded dependency-free parser; no regex or recursive/backtracking matcher.
#[derive(Debug)]
pub struct Rfc9309 {
    /// Groups in file order; matching groups are merged, not first-picked.
    groups: Vec<Group>,
}
impl Rfc9309 {
    /// Parse at most 500 KiB and the stricter policy byte ceiling.
    /// Unknown records/comments are ignored; malformed recognized rules refuse.
    ///
    /// # Errors
    /// Oversized, invalid UTF-8, product tokens or path escapes refuse.
    pub fn parse(bytes: &[u8], max_bytes: u64) -> Result<Self, Refusal> {
        if bytes.len() as u64 > max_bytes.min(512_000) {
            return Err(Refusal::Invalid);
        }
        let text = from_utf8(bytes).map_err(|_| Refusal::Invalid)?;
        let mut groups = Vec::new();
        let mut group = Group::default();
        for line in text.trim_start_matches('\u{feff}').lines() {
            parse_line(line, &mut group, &mut groups)?;
        }
        groups.push(group);
        Ok(Self { groups })
    }
}
/// Consume one record; unknown extensions do not terminate a group.
fn parse_line(line: &str, group: &mut Group, groups: &mut Vec<Group>) -> Result<(), Refusal> {
    let line = line.split('#').next().unwrap_or_default().trim();
    let Some((key, value)) = line.split_once(':') else {
        return Ok(());
    };
    let value = value.trim();
    if key.trim().eq_ignore_ascii_case("user-agent") {
        if group.has_directive {
            groups.push(take(group));
        }
        if value != "*" && !product_token(value) {
            return Err(Refusal::Invalid);
        }
        group.agents.push(value.to_ascii_lowercase());
        return Ok(());
    }
    if group.agents.is_empty() {
        return Ok(());
    }
    let allow = key.trim().eq_ignore_ascii_case("allow");
    if !allow && !key.trim().eq_ignore_ascii_case("disallow") {
        return Ok(());
    }
    group.has_directive = true;
    if value.is_empty() {
        return Ok(());
    }
    if !value.starts_with('/') {
        return Err(Refusal::Invalid);
    }
    group.rules.push(Rule {
        pattern: normalize(value)?,
        allow,
    });
    Ok(())
}
impl RobotsRules for Rfc9309 {
    fn allowed(&self, agent: &str, identity: &FetchIdentity) -> bool {
        if !product_token(agent) {
            return false;
        }
        let agent = agent.to_ascii_lowercase();
        let specific = self
            .groups
            .iter()
            .any(|group| group.agents.contains(&agent));
        let selected = if specific { agent.as_str() } else { "*" };
        let url = identity.url();
        let mut path = url.path().to_owned();
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        }
        let Ok(path) = normalize(&path) else {
            return false;
        };
        self.groups
            .iter()
            .filter(|group| group.agents.iter().any(|agent| agent == selected))
            .flat_map(|group| &group.rules)
            .filter(|rule| wildcard_match(&rule.pattern, &path))
            .max_by_key(|rule| (rule.pattern.len(), rule.allow))
            .is_none_or(|rule| rule.allow)
    }
}
/// RFC product tokens consist of letters, underscores and hyphens.
fn product_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'_' || byte == b'-')
}
/// Normalize RFC octets: decode unreserved escapes, uppercase other escapes and
/// percent-encode raw non-ASCII UTF-8. Encoded reserved octets stay distinct.
fn normalize(text: &str) -> Result<String, Refusal> {
    let mut result = String::new();
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes
                .next()
                .and_then(|byte| char::from(byte).to_digit(16))
                .ok_or(Refusal::Invalid)?;
            let low = bytes
                .next()
                .and_then(|byte| char::from(byte).to_digit(16))
                .ok_or(Refusal::Invalid)?;
            let decoded = u8::try_from(high * 16 + low).map_err(|_| Refusal::Invalid)?;
            if decoded.is_ascii_alphanumeric() || b"-._~".contains(&decoded) {
                result.push(char::from(decoded));
            } else {
                push_escape(&mut result, decoded);
            }
        } else if !byte.is_ascii() {
            push_escape(&mut result, byte);
        } else if byte.is_ascii_control() {
            return Err(Refusal::Invalid);
        } else {
            result.push(char::from(byte));
        }
    }
    Ok(result)
}
/// One byte to uppercase percent encoding, without formatting allocations.
fn push_escape(result: &mut String, byte: u8) {
    result.push('%');
    result.push(
        char::from_digit(u32::from(byte / 16), 16)
            .unwrap_or_default()
            .to_ascii_uppercase(),
    );
    result.push(
        char::from_digit(u32::from(byte % 16), 16)
            .unwrap_or_default()
            .to_ascii_uppercase(),
    );
}
/// Consume literal segments only forwards. An anchored final segment is fixed
/// at the suffix first, so ambiguous stars never cause backtracking.
fn wildcard_match(pattern: &str, path: &str) -> bool {
    let (pattern, anchored) = pattern
        .strip_suffix('$')
        .map_or((pattern, false), |pattern| (pattern, true));
    let mut segments = pattern.split('*');
    let Some(first) = segments.next() else {
        return true;
    };
    let Some(mut rest) = path.strip_prefix(first) else {
        return false;
    };
    let mut remaining = segments.peekable();
    if remaining.peek().is_none() {
        return !anchored || rest.is_empty();
    }
    while let Some(segment) = remaining.next() {
        if anchored && remaining.peek().is_none() {
            return rest.ends_with(segment);
        }
        let Some(position) = rest.find(segment) else {
            return false;
        };
        let Some(suffix) = rest.get(position + segment.len()..) else {
            return false;
        };
        rest = suffix;
    }
    true
}
/// Origin/agent-bound rules evidence supplied only after N09's admitted fetch.
#[derive(Debug)]
pub struct RobotsCache {
    /// Canonical origin; a redirected response still belongs to the first origin.
    origin: String,
    /// Exact configured agent prevents stale reuse after an identity change.
    agent: String,
    /// Trusted host observation time, not remote content time.
    fetched_ms: u64,
    /// Original policy TTL; later policies may only tighten this cached entry.
    ttl_ms: u64,
    /// Size checked again if policy tightens while the entry is cached.
    bytes: u64,
    /// None is an unreadable/invalid response, never absence of rules.
    rules: Option<Box<dyn RobotsRules>>,
}
impl RobotsCache {
    /// Retain an admitted final HTTP response. 404/410 mean no rules; every other
    /// non-2xx response denies. No HTTP client or redirect bypass lives here.
    #[must_use]
    pub fn response(
        identity: &FetchIdentity,
        policy: &Robots,
        status: u16,
        body: &[u8],
        now_ms: u64,
    ) -> Self {
        let rules = match status {
            200..=299 => Rfc9309::parse(body, policy.rules_max_bytes.get()).ok(),
            404 | 410 => Rfc9309::parse(b"", policy.rules_max_bytes.get()).ok(),
            _ => None,
        };
        Self {
            rules: rules.map(|rules| Box::new(rules) as Box<dyn RobotsRules>),
            bytes: if status == 404 || status == 410 {
                0
            } else {
                body.len() as u64
            },
            ..Self::unreadable(identity, policy, now_ms)
        }
    }
    /// Substitute parsed rules from an admitted response under the same cache
    /// gate. The adapter owns parsing; the caller supplies its actual input size.
    /// This creates no authority and does not weaken origin/agent/TTL checks.
    #[must_use]
    pub fn parsed(
        identity: &FetchIdentity,
        policy: &Robots,
        rules: Box<dyn RobotsRules>,
        bytes: u64,
        now_ms: u64,
    ) -> Self {
        Self {
            rules: if bytes <= policy.rules_max_bytes.get().min(512_000) {
                Some(rules)
            } else {
                None
            },
            bytes,
            ..Self::unreadable(identity, policy, now_ms)
        }
    }
    /// Missing, timeout or network-error evidence is always fail-closed.
    #[must_use]
    pub fn unreadable(identity: &FetchIdentity, policy: &Robots, now_ms: u64) -> Self {
        Self {
            origin: identity.url().origin().ascii_serialization(),
            agent: policy.agent.clone(),
            fetched_ms: now_ms,
            ttl_ms: policy.cache_ttl_ms.get().min(86_400_000),
            bytes: 0,
            rules: None,
        }
    }
    /// Enforce current source policy before every request. Overrides need the
    /// exact current `OA4b` receipt, including expiry/revocation checks by the port.
    ///
    /// # Errors
    /// Wrong origin/agent, stale evidence, missing/expired override or denial.
    pub fn check(
        &self,
        identity: &FetchIdentity,
        policy: &Robots,
        now_ms: u64,
        authority: &dyn RobotsOverride,
    ) -> Result<(), Refusal> {
        let origin = identity.url().origin().ascii_serialization();
        if self.origin != origin || self.agent != policy.agent {
            return Err(Refusal::Access);
        }
        if let Some(receipt) = &policy.r#override {
            let request = OverrideRequest {
                operation: "robots_override",
                source_id: identity.source_id(),
                origin: &origin,
                receipt,
                now_ms,
            };
            return match authority.decide(&request) {
                OverrideDecision::Granted(current) if current == *receipt => Ok(()),
                _ => Err(Refusal::Access),
            };
        }
        let age = now_ms.checked_sub(self.fetched_ms).ok_or(Refusal::Access)?;
        if age >= self.ttl_ms.min(policy.cache_ttl_ms.get())
            || self.bytes > policy.rules_max_bytes.get()
        {
            return Err(Refusal::Access);
        }
        if self
            .rules
            .as_ref()
            .is_some_and(|rules| rules.allowed(&policy.agent, identity))
        {
            Ok(())
        } else {
            Err(Refusal::Access)
        }
    }
}
