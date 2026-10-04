//! Deny-only address resources plus a non-removable special-purpose floor.
use crate::{policy::resource::Resource, refusal::Refusal};
use maestro_knowledge::strict_json;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv6Addr},
};

/// Only this strict address resource version is executable.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Schema {
    /// Reviewed deny-only snapshot.
    #[serde(rename = "maestro-address-table/1")]
    V1,
}
/// Wire address data never contains permission or an allow-list.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddressResource {
    /// Common immutable ownership and collection identity.
    #[serde(flatten)]
    pub(crate) resource: Resource<Schema>,
    /// Informational snapshot description, never a pin or authority.
    snapshot: String,
    /// Canonical CIDRs; additional denials only.
    deny_prefixes: Vec<String>,
}
/// Immutable executable classification, not permission to connect.
#[derive(Debug, Clone)]
pub struct AddressTable {
    /// Floor and reviewed supplemental denials, with no remove operation.
    denied: Vec<Prefix>,
}
impl AddressResource {
    /// Parse and validate all resource data under the common defensive ceiling.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, Refusal> {
        strict_json::parse(bytes).map_err(|_| Refusal::Invalid)
    }
    /// Compile canonical prefixes; floor denials are always prepended.
    pub(crate) fn compile(&self) -> Result<AddressTable, Refusal> {
        if self.snapshot.is_empty()
            || self.snapshot.len() > 4096
            || self.snapshot.chars().any(char::is_control)
            || self.deny_prefixes.len() > 1000
        {
            return Err(Refusal::Invalid);
        }
        let mut unique = BTreeSet::new();
        let mut denied = super::floor::PREFIXES
            .iter()
            .map(|prefix| Prefix::parse(prefix))
            .collect::<Result<Vec<_>, _>>()?;
        for prefix in &self.deny_prefixes {
            if !unique.insert(prefix) {
                return Err(Refusal::Invalid);
            }
            denied.push(Prefix::parse(prefix)?);
        }
        Ok(AddressTable { denied })
    }
}
impl AddressTable {
    /// Classify a canonical resolver answer. No private-unicast exception exists.
    ///
    /// # Errors
    /// Special-purpose, supplemental, ambiguous, embedded or unallocated answers refuse.
    pub fn admit(&self, text: &str) -> Result<IpAddr, Refusal> {
        let address: IpAddr = text.parse().map_err(|_| Refusal::Invalid)?;
        if address.to_string() != text {
            return Err(Refusal::Invalid);
        }
        if let IpAddr::V6(ip) = address {
            // All known transition prefixes are special-purpose and denied even
            // for public embedded targets. ISATAP also embeds IPv4 outside them.
            if !global_v6(ip) || isatap(ip) {
                return Err(Refusal::Access);
            }
        }
        if self.denied.iter().any(|prefix| prefix.contains(address)) {
            return Err(Refusal::Access);
        }
        Ok(address)
    }
}
/// IPv6 currently allocated global-unicast space; policy may only narrow it.
fn global_v6(ip: Ipv6Addr) -> bool {
    u128::from(ip) >> 125 == 1
}
/// Unsupported ISATAP embedding cannot smuggle an IPv4 target in global space.
fn isatap(ip: Ipv6Addr) -> bool {
    let interface = (u128::from(ip) >> 32) & 0xffff_ffff;
    interface == 0x0000_5efe || interface == 0x0200_5efe
}
/// Canonical network bits and prefix width in one address family.
#[derive(Debug, Clone)]
struct Prefix {
    /// Canonical network address with zero host bits.
    network: IpAddr,
    /// Number of network bits in this family.
    width: u32,
}
impl Prefix {
    /// Refuse host bits, alternate spelling and invalid widths.
    fn parse(text: &str) -> Result<Self, Refusal> {
        let (network, width) = text.split_once('/').ok_or(Refusal::Invalid)?;
        let network: IpAddr = network.parse().map_err(|_| Refusal::Invalid)?;
        let width: u32 = width.parse().map_err(|_| Refusal::Invalid)?;
        let prefix = Self { network, width };
        if text != format!("{network}/{width}") || !prefix.canonical() {
            return Err(Refusal::Invalid);
        }
        Ok(prefix)
    }
    /// Host bits must be zero; /0 avoids an oversized shift.
    fn canonical(&self) -> bool {
        match self.network {
            IpAddr::V4(ip) => self.width <= 32 && u32::from(ip) & !mask32(self.width) == 0,
            IpAddr::V6(ip) => self.width <= 128 && u128::from(ip) & !mask128(self.width) == 0,
        }
    }
    /// Compare only addresses in the prefix's own family.
    fn contains(&self, address: IpAddr) -> bool {
        match (self.network, address) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                u32::from(network) == u32::from(ip) & mask32(self.width)
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                u128::from(network) == u128::from(ip) & mask128(self.width)
            }
            _ => false,
        }
    }
}
/// Safe IPv4 mask, including the zero-width network.
fn mask32(width: u32) -> u32 {
    u32::MAX
        .checked_shl(32_u32.saturating_sub(width))
        .unwrap_or(0)
}
/// Safe IPv6 mask, including the zero-width network.
fn mask128(width: u32) -> u128 {
    u128::MAX
        .checked_shl(128_u32.saturating_sub(width))
        .unwrap_or(0)
}

#[cfg(test)]
mod mutation_tests {
    use super::AddressResource;
    #[test]
    fn s6t_address_resource_exact_snapshot_and_prefix_ceilings() {
        let mut resource =
            AddressResource::parse(include_bytes!("../../tests/fixtures/address-table.json"))
                .unwrap();
        resource.snapshot = "x".repeat(4096);
        resource.deny_prefixes = (0..1000)
            .map(|i| format!("10.{}.{}.0/24", i / 256, i % 256))
            .collect();
        assert!(resource.compile().is_ok());
    }
}
