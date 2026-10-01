//! Read-only acquisition authority; manifests and registry evidence grant no access.
use super::shape::{checked_url, valid_id};
use crate::refusal::Refusal;
use maestro_kernel::scope::Scope;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{self, Debug},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Separately authorized effects; an ordinary fetch never overrides robots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Read the exact source/account target.
    Fetch,
    /// Exceptional, separately granted robots override for the exact target.
    RobotsOverride,
}
/// Exact conjunctive scope; no prefix, wildcard or implicit account matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Collection visibility scope.
    pub scope: String,
    /// Exact source identity.
    pub source: String,
    /// Exact isolated account role (including an explicit public role).
    pub account: String,
    /// Exact HTTPS resource; transfer credentials are not grant targets.
    pub resource: String,
}
impl Target {
    /// Validate an exact target without performing any effect.
    ///
    /// # Errors
    /// Invalid scope, logical ID or credential-bearing URL refuses.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.scope.parse::<Scope>().is_err()
            || !valid_id(&self.source)
            || !valid_id(&self.account)
            || checked_url(&self.resource).is_none()
            || self.resource.contains(['?', '#'])
        {
            return Err(Refusal::Invalid);
        }
        Ok(())
    }
    /// The same N03 canonical URL spelling gives the same exact authority target.
    ///
    /// # Errors
    /// Invalid scope, account or URL refuses without effects.
    pub fn canonical(&self) -> Result<Self, Refusal> {
        self.validate()?;
        let mut target = self.clone();
        checked_url(&self.resource)
            .ok_or(Refusal::Invalid)?
            .as_str()
            .clone_into(&mut target.resource);
        Ok(target)
    }
}
/// Stored only by the separately authenticated writer; never policy evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// Stable grant ID for audit and revocation.
    pub id: String,
    /// Exact platform-assigned principal, never a model-supplied identity.
    pub principal: String,
    /// Separately approved effect.
    pub operation: Operation,
    /// Conjunctive source/account/collection/resource binding.
    pub target: Target,
    /// UTC RFC3339 expiry at whole-second precision.
    pub expires_at: String,
}
impl Grant {
    /// Validate this record and return its authority-clock expiry.
    ///
    /// # Errors
    /// Missing/invalid exact scope, principal or expiry refuses.
    pub fn expiry(&self) -> Result<SystemTime, Refusal> {
        self.target.validate()?;
        if !valid_id(&self.id) || !valid_id(&self.principal) {
            return Err(Refusal::Invalid);
        }
        expiry(&self.expires_at)
    }
    /// Validate and canonicalize the exact confirmed record before storage or matching.
    ///
    /// # Errors
    /// Invalid identity, target or expiry refuses.
    pub fn canonical(&self) -> Result<Self, Refusal> {
        self.expiry()?;
        let mut grant = self.clone();
        grant.target = self.target.canonical()?;
        Ok(grant)
    }
    /// One exact read-only decision against the authority's current clock.
    ///
    /// # Errors
    /// Invalid, expired or mismatched grants refuse; callers recheck per dispatch.
    pub fn decide(
        &self,
        principal: &str,
        operation: Operation,
        target: &Target,
        now: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        let expiry = self.expiry()?;
        if self.principal != principal
            || self.operation != operation
            || self.target.canonical()? != target.canonical()?
        {
            return Err(Refusal::Access.into());
        }
        if expiry <= now {
            return Err(AuthorityRefusal::Expired {
                grant_id: self.id.clone(),
            });
        }
        Ok(Permit {
            grant_id: self.id.clone(),
        })
    }
}
/// Evidence for this dispatch only; not a cached or transferable entitlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permit {
    /// Exact current grant whose scope matched.
    pub grant_id: String,
}
/// Read-only refusal; expiry names only an exact matched grant, never another scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityRefusal {
    /// Ordinary invalid, inaccessible or unqualified authority.
    Refused(Refusal),
    /// The matched grant has expired; acquisition owns its receipt, not this port.
    Expired {
        /// Exact expired grant's logical audit identity.
        grant_id: String,
    },
}
impl From<Refusal> for AuthorityRefusal {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}
impl fmt::Display for AuthorityRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never echo private source/account/target data through an error diagnostic.
        match self {
            Self::Refused(reason) => fmt::Display::fmt(reason, formatter),
            Self::Expired { .. } => formatter.write_str("authority expired"),
        }
    }
}
impl Error for AuthorityRefusal {}

/// Replaceable read-only authority adapter. No create/edit/revoke API is exposed.
pub trait Authority: Debug {
    /// Decide against current authority state and its clock, never a manifest approval.
    ///
    /// # Errors
    /// Missing separation, revoked/expired authority or any binding mismatch refuses.
    fn decide(
        &self,
        principal: &str,
        operation: Operation,
        target: &Target,
        now: SystemTime,
    ) -> Result<Permit, AuthorityRefusal>;
}
/// Explicitly disabled/unqualified hosts cannot silently use synthetic grants.
#[derive(Debug)]
pub struct UnqualifiedAuthority;
impl Authority for UnqualifiedAuthority {
    fn decide(
        &self,
        _principal: &str,
        _operation: Operation,
        _target: &Target,
        _now: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        Err(Refusal::Unqualified.into())
    }
}
/// Parse the documented fixed-width UTC spelling without a time dependency.
fn expiry(text: &str) -> Result<SystemTime, Refusal> {
    // The shared strict date decoder checks leap years, bounds and exact fields.
    let encoded = serde_json::to_string(text).map_err(|_| Refusal::Invalid)?;
    let mut decoder = serde_json::Deserializer::from_str(&encoded);
    super::shape::time(&mut decoder).map_err(|_| Refusal::Invalid)?;
    let parts: Vec<u64> = text
        .split(['-', 'T', ':', 'Z'])
        .filter(|part| !part.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| Refusal::Invalid)?;
    let [year, month, day, hour, minute, second] = parts.as_slice() else {
        return Err(Refusal::Invalid);
    };
    let mut days = 0;
    for previous in 0..*year {
        days += year_days(previous);
    }
    for previous in 1..*month {
        days += month_days(*year, previous);
    }
    days += day - 1;
    // Gregorian days from year zero to 1970-01-01, including year zero's leap day.
    let epoch = 719_528 * 86_400;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second;
    if seconds < epoch {
        UNIX_EPOCH
            .checked_sub(Duration::from_secs(epoch - seconds))
            .ok_or(Refusal::Invalid)
    } else {
        UNIX_EPOCH
            .checked_add(Duration::from_secs(seconds - epoch))
            .ok_or(Refusal::Invalid)
    }
}
/// Number of days in a Gregorian year.
fn year_days(year: u64) -> u64 {
    if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
        366
    } else {
        365
    }
}
/// Shared leap-year arithmetic, after the strict decoder validated the month.
fn month_days(year: u64, month: u64) -> u64 {
    match month {
        2 => year_days(year) - 337,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
