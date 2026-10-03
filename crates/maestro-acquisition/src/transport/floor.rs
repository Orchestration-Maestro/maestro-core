//! Non-removable IANA snapshot (2025-10-09), multicast and metadata denials.
/// Parents cover nested records; `global_v6` denies ranges outside `2000::/3`.
/// Tests retain the full registry fixture and prove coverage without supplemental denials.
pub(super) const PREFIXES: &[&str] = &[
    "0.0.0.0/8",
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.0.0.0/24",
    "192.0.2.0/24",
    "192.31.196.0/24",
    "192.52.193.0/24",
    "192.88.99.0/24",
    "192.168.0.0/16",
    "192.175.48.0/24",
    "198.18.0.0/15",
    "198.51.100.0/24",
    "203.0.113.0/24",
    "240.0.0.0/4",
    "2001::/23",
    "2001:db8::/32",
    "2002::/16",
    "2620:4f:8000::/48",
    "3fff::/20",
    "224.0.0.0/4",
    "168.63.129.16/32",
];

#[cfg(test)]
mod tests {
    use super::PREFIXES;
    use crate::{refusal::Refusal, transport::address::AddressResource};
    use serde_json::{Value, json};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    /// First, last and midpoint; singleton prefixes repeat their only address.
    fn samples(prefix: &str) -> [IpAddr; 3] {
        let (address, width) = prefix.split_once('/').unwrap();
        let width = width.parse().unwrap();
        match address.parse::<IpAddr>().unwrap() {
            IpAddr::V4(ip) => {
                let host = u32::MAX.checked_shr(width).unwrap_or(0);
                [
                    ip,
                    Ipv4Addr::from(u32::from(ip) | host),
                    Ipv4Addr::from(u32::from(ip) | (host >> 1)),
                ]
                .map(IpAddr::V4)
            }
            IpAddr::V6(ip) => {
                let host = u128::MAX.checked_shr(width).unwrap_or(0);
                [
                    ip,
                    Ipv6Addr::from(u128::from(ip) | host),
                    Ipv6Addr::from(u128::from(ip) | (host >> 1)),
                ]
                .map(IpAddr::V6)
            }
        }
    }
    /// Independent registry widths catch floor narrowing without data masking it.
    #[test]
    fn n08_floor_covers_full_registry_without_supplemental_denials() {
        let mut registry: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/address-table.json")).unwrap();
        let prefixes = registry["deny_prefixes"].take();
        registry["deny_prefixes"] = json!([]);
        let table = AddressResource::parse(&serde_json::to_vec(&registry).unwrap())
            .unwrap()
            .compile()
            .unwrap();
        for prefix in prefixes.as_array().unwrap() {
            for address in samples(prefix.as_str().unwrap()) {
                assert_eq!(
                    table.admit(&address.to_string()),
                    Err(Refusal::Access),
                    "floor {prefix}, address {address}"
                );
            }
        }
    }
    /// Redundant nested ranges cannot have independently observable denials.
    #[test]
    fn n08_floor_has_no_redundant_nested_prefixes() {
        for (index, prefix) in PREFIXES.iter().enumerate() {
            let [first, last, _] = samples(prefix);
            if let (IpAddr::V6(first), IpAddr::V6(last)) = (first, last) {
                assert!(
                    u128::from(first) >> 125 == 1 && u128::from(last) >> 125 == 1,
                    "redundant {prefix} outside 2000::/3"
                );
            }
            for (other_index, other) in PREFIXES.iter().enumerate() {
                let [other_first, other_last, _] = samples(other);
                assert!(
                    index == other_index || first < other_first || last > other_last,
                    "redundant {prefix} within {other}"
                );
            }
        }
    }
}
