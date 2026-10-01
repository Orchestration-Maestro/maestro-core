//! N08 uses real admission with explicit synthetic resolver/connection adapters.
use super::{n07_parse_url_identity_and_denial_precedence::checked, support};
use maestro_acquisition::policy::identity::FetchIdentity;
use maestro_acquisition::{
    Refusal,
    transport::{
        address::AddressTable,
        connect::{CheckedDestination, OriginCredentials, PinnedTransport, Resolver, connect},
    },
};
use reqwest::{
    Url,
    header::{AUTHORIZATION, COOKIE, HeaderMap, HeaderValue},
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
};
use tokio::runtime::Builder;

/// Address engine under an actual reviewed policy closure.
fn table() -> AddressTable {
    let (collection, catalog) = support::fixture();
    checked(collection, &catalog)
        .unwrap()
        .address_table()
        .clone()
}
/// Count resolutions, making a second resolution observable.
#[derive(Debug)]
struct Answers {
    calls: Cell<usize>,
    addresses: Vec<String>,
}
impl Resolver for Answers {
    fn resolve(&self, _host: &str) -> Result<Vec<String>, Refusal> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.addresses.clone())
    }
}
/// Deny every prefix independently, including special globally-reachable ranges.
#[test]
fn n08_every_snapshot_prefix_denies_without_effects() {
    let table = table();
    let value: Value =
        serde_json::from_str(include_str!("../fixtures/address-table.json")).unwrap();
    for prefix in value["deny_prefixes"].as_array().unwrap() {
        let (address, width) = prefix.as_str().unwrap().split_once('/').unwrap();
        let width: u32 = width.parse().unwrap();
        let first = address.parse::<IpAddr>().unwrap();
        let last = match first {
            IpAddr::V4(ip) => IpAddr::V4(Ipv4Addr::from(
                u32::from(ip) | u32::MAX.checked_shr(width).unwrap_or(0),
            )),
            IpAddr::V6(ip) => IpAddr::V6(Ipv6Addr::from(
                u128::from(ip) | u128::MAX.checked_shr(width).unwrap_or(0),
            )),
        };
        assert_eq!(table.admit(address), Err(Refusal::Access), "{prefix}");
        assert_eq!(
            table.admit(&last.to_string()),
            Err(Refusal::Access),
            "last {prefix}"
        );
    }
    for address in [
        "8.8.8.8",
        "100.63.255.255",
        "100.128.0.0",
        "172.15.255.255",
        "172.32.0.0",
        "223.255.255.255",
        "2606:4700:4700::1111",
        "3ffe:ffff::1",
    ] {
        assert_eq!(
            table.admit(address),
            Ok(address.parse::<IpAddr>().unwrap()),
            "{address}"
        );
    }
}
/// Removing all data denials still cannot weaken the floor; data may add denials.
#[test]
fn n08_data_only_adds_denials() {
    let (mut collection, mut catalog) = support::fixture();
    let mut value = support::value(&catalog, "addresses");
    value["deny_prefixes"] = json!(["8.8.8.0/24"]);
    support::put(&mut catalog, "addresses", &value);
    support::rebind(&mut collection, &mut catalog);
    let policy = checked(collection, &catalog).unwrap();
    assert_eq!(
        policy.address_table().admit("8.8.8.8"),
        Err(Refusal::Access)
    );
    let original: Value =
        serde_json::from_str(include_str!("../fixtures/address-table.json")).unwrap();
    for prefix in original["deny_prefixes"].as_array().unwrap() {
        let address = prefix.as_str().unwrap().split_once('/').unwrap().0;
        assert_eq!(
            policy.address_table().admit(address),
            Err(Refusal::Access),
            "floor {prefix}"
        );
    }
    assert!(policy.address_table().admit("8.8.9.1").is_ok());
}
/// Empty supplemental data preserves the floor; /0 can deliberately deny all.
#[test]
fn n08_empty_and_full_width_prefixes() {
    for (prefixes, permitted) in [
        (json!([]), true),
        (json!(["0.0.0.0/0", "::/0"]), false),
        (json!(["8.8.8.8/32", "2606:4700::1/128"]), false),
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut value = support::value(&catalog, "addresses");
        value["deny_prefixes"] = prefixes;
        support::put(&mut catalog, "addresses", &value);
        support::rebind(&mut collection, &mut catalog);
        let policy = checked(collection, &catalog).unwrap();
        for address in ["8.8.8.8", "2606:4700::1"] {
            assert_eq!(policy.address_table().admit(address).is_ok(), permitted);
        }
        assert!(policy.address_table().admit("127.0.0.1").is_err());
    }
}
/// Strict address data must be executable, not an opaque reviewed JSON blob.
#[test]
fn n08_table_refuses_bad_schema_cidrs_and_evidence() {
    let oversized = (0..1001)
        .map(|n| format!("{}/32", Ipv4Addr::from(0x0800_0000 + n)))
        .collect::<Vec<_>>();
    for change in [
        json!({"id":"other-addresses"}),
        json!({"snapshot":""}),
        json!({"snapshot":"x".repeat(4097)}),
        json!({"snapshot":"bad\nlabel"}),
        json!({"deny_prefixes": oversized}),
        json!({"deny_prefixes":["8.8.8.1/24"]}),
        json!({"deny_prefixes":["2606:4700:0:0::/32"]}),
        json!({"deny_prefixes":["8.8.8.0/33"]}),
        json!({"deny_prefixes":["2606:4700::1/32"]}),
        json!({"deny_prefixes":["2606:4700::/129"]}),
        json!({"deny_prefixes":["8.8.8.0/024"]}),
        json!({"allow_prefixes":[]}),
        json!({"schema":"maestro-address-table/2"}),
        json!({"deny_prefixes":["8.8.8.0/24", "8.8.8.0/24"]}),
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut value = support::value(&catalog, "addresses");
        for (key, replacement) in change.as_object().unwrap() {
            value[key] = replacement.clone();
        }
        support::put(&mut catalog, "addresses", &value);
        support::rebind(&mut collection, &mut catalog);
        assert!(checked(collection, &catalog).is_err(), "{change}");
    }
    let (collection, mut catalog) = support::fixture();
    catalog.0.get_mut("addresses").unwrap().bytes.push(b' ');
    assert!(checked(collection, &catalog).is_err());
}
/// All translation/tunnel forms refuse, even when embedding a public target.
#[test]
fn n08_embedded_zone_and_unallocated_addresses_refuse() {
    for address in [
        "::ffff:127.0.0.1",
        "::ffff:8.8.8.8",
        "::127.0.0.1",
        "64:ff9b::a00:1",
        "64:ff9b:1::808:808",
        "2002:7f00:1::",
        "2002:808:808::",
        "2001:0:4136:e378:8000:63bf:3fff:fdd2",
        "2606:4700::5efe:7f00:1",
        "2606:4700::200:5efe:808:808",
        "fe80::1%eth0",
        "fe80::1%25eth0",
        "4000::1",
        "fec0::1",
        "169.254.169.254",
        "100.100.100.200",
    ] {
        assert!(table().admit(address).is_err(), "{address}");
    }
}
/// A mixed DNS answer refuses as a whole, and selection is immutable after DNS.
#[test]
fn n08_checks_every_candidate_and_pins_without_rebinding() {
    let (collection, catalog) = support::fixture();
    let policy = checked(collection, &catalog).unwrap();
    let source = policy.policy().sources.first().unwrap();
    let identity = FetchIdentity::parse(source, "https://garden.example/docs/start").unwrap();
    for addresses in [vec![], vec!["8.8.8.8", "127.0.0.1"], vec!["fe80::1%eth0"]] {
        let resolver = Answers {
            calls: Cell::new(0),
            addresses: addresses.into_iter().map(String::from).collect(),
        };
        assert!(CheckedDestination::resolve(&identity, policy.address_table(), &resolver).is_err());
    }
    let resolver = Answers {
        calls: Cell::new(0),
        addresses: vec!["8.8.8.8".into(), "1.1.1.1".into()],
    };
    let destination =
        CheckedDestination::resolve(&identity, policy.address_table(), &resolver).unwrap();
    assert_eq!(destination.socket().to_string(), "8.8.8.8:443");
    assert_eq!(destination.hostname(), "garden.example");
    assert_eq!(resolver.calls.get(), 1);
    assert_eq!(
        destination.socket().ip(),
        "8.8.8.8".parse::<IpAddr>().unwrap()
    );
}
/// A synthetic wire adapter records only effects after core address admission.
#[derive(Debug, Default)]
struct Wire {
    /// Observed connections, never merely attempted dispatches.
    effects: Cell<usize>,
}
impl PinnedTransport for Wire {
    type Connection = SocketAddr;
    fn connect(
        &self,
        destination: CheckedDestination,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Refusal>> + Send + '_>> {
        self.effects.set(self.effects.get() + 1);
        Box::pin(async move { Ok(destination.socket()) })
    }
}
/// An old successful connection cannot hide a denied new DNS or policy result.
#[test]
fn n08_dispatch_rechecks_dns_and_pool_changes_before_effects() {
    let (mut collection, mut catalog) = support::fixture();
    let policy = checked(collection.clone(), &catalog).unwrap();
    let identity = FetchIdentity::parse(
        policy.policy().sources.first().unwrap(),
        "https://garden.example/docs/start",
    )
    .unwrap();
    let wire = Wire::default();
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    for (addresses, permitted) in [
        (vec!["8.8.8.8"], true),
        (vec!["8.8.8.8", "127.0.0.1"], false),
        (vec!["1.1.1.1"], true),
    ] {
        let resolver = Answers {
            calls: Cell::new(0),
            addresses: addresses.into_iter().map(String::from).collect(),
        };
        let before = wire.effects.get();
        let result = runtime.block_on(connect(&identity, policy.address_table(), &resolver, &wire));
        assert_eq!(result.is_ok(), permitted);
        assert_eq!(wire.effects.get() - before, usize::from(permitted));
        assert_eq!(resolver.calls.get(), 1);
        if let Ok(socket) = result {
            assert_eq!(
                socket.ip().to_string(),
                resolver.addresses.first().unwrap().as_str()
            );
        }
    }
    let mut table = support::value(&catalog, "addresses");
    table["deny_prefixes"] = json!(["8.8.8.0/24"]);
    support::put(&mut catalog, "addresses", &table);
    support::rebind(&mut collection, &mut catalog);
    let tightened = checked(collection, &catalog).unwrap();
    let resolver = Answers {
        calls: Cell::new(0),
        addresses: vec!["8.8.8.8".into()],
    };
    let before = wire.effects.get();
    assert!(
        runtime
            .block_on(connect(
                &identity,
                tightened.address_table(),
                &resolver,
                &wire
            ))
            .is_err()
    );
    assert_eq!(wire.effects.get(), before);
}
/// Failed DNS must refuse instead of inventing a public fallback destination.
#[derive(Debug)]
struct FailedResolver;
impl Resolver for FailedResolver {
    fn resolve(&self, _hostname: &str) -> Result<Vec<String>, Refusal> {
        Err(Refusal::Access)
    }
}
/// Resolver errors stop dispatch before the transport sees any destination.
#[test]
fn n08_dns_failure_refuses_without_wire_effects() {
    let (collection, catalog) = support::fixture();
    let policy = checked(collection, &catalog).unwrap();
    let identity = FetchIdentity::parse(
        policy.policy().sources.first().unwrap(),
        "https://garden.example/docs/start",
    )
    .unwrap();
    let wire = Wire::default();
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    assert_eq!(
        runtime.block_on(connect(
            &identity,
            policy.address_table(),
            &FailedResolver,
            &wire
        )),
        Err(Refusal::Access)
    );
    assert_eq!(wire.effects.get(), 0);
}
/// Socket selection retains an explicitly admitted non-default HTTPS port.
#[test]
fn n08_pins_declared_non_default_https_port() {
    let (mut collection, mut catalog) = support::fixture();
    let mut value = support::value(&catalog, "policy");
    value["sources"][0]["origins"][0]["port"] = json!(8443);
    value["sources"][0]["seeds"][0] = json!("https://garden.example:8443/docs/start");
    support::put(&mut catalog, "policy", &value);
    support::rebind(&mut collection, &mut catalog);
    let policy = checked(collection, &catalog).unwrap();
    let identity = FetchIdentity::parse(
        policy.policy().sources.first().unwrap(),
        "https://garden.example:8443/docs/start",
    )
    .unwrap();
    let resolver = Answers {
        calls: Cell::new(0),
        addresses: vec!["8.8.8.8".into()],
    };
    let destination =
        CheckedDestination::resolve(&identity, policy.address_table(), &resolver).unwrap();
    assert_eq!(destination.socket().port(), 8443);
}
/// Exact defensive limits and canonical parsing run before any wire effect.
#[test]
fn n08_resolver_limits_and_canonical_addresses() {
    let table = table();
    for address in [
        "8.008.8.8",
        "0x08080808",
        "134744072",
        "2606:4700:0:0::1",
        "2606:4700::ABCD",
        "8.8.8.8%1",
        " 8.8.8.8",
    ] {
        assert!(table.admit(address).is_err(), "{address}");
    }
    let (collection, catalog) = support::fixture();
    let policy = checked(collection, &catalog).unwrap();
    let identity = FetchIdentity::parse(
        policy.policy().sources.first().unwrap(),
        "https://garden.example/docs/start",
    )
    .unwrap();
    for count in [256, 257] {
        let resolver = Answers {
            calls: Cell::new(0),
            addresses: vec!["8.8.8.8".into(); count],
        };
        assert_eq!(
            CheckedDestination::resolve(&identity, &table, &resolver).is_ok(),
            count == 256
        );
    }
}
/// Origin-bound headers never survive a scheme, hostname or port change.
#[test]
fn n08_credentials_are_exact_origin_bound() {
    let (collection, catalog) = support::fixture();
    let policy = checked(collection, &catalog).unwrap();
    let url = FetchIdentity::parse(
        policy.policy().sources.first().unwrap(),
        "https://garden.example/docs",
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("synthetic-secret"));
    headers.insert(COOKIE, HeaderValue::from_static("synthetic-cookie"));
    let credentials = OriginCredentials::new(&url, headers);
    assert_eq!(format!("{credentials:?}"), "OriginCredentials([redacted])");
    let forwarded = credentials.for_url(url.url());
    assert!(
        forwarded.values().all(HeaderValue::is_sensitive),
        "origin credential values are not Debug-redacted"
    );
    let debug = format!("{forwarded:?}");
    assert!(!debug.contains("synthetic-secret"));
    assert!(!debug.contains("synthetic-cookie"));
    for target in [
        "https://other.example/docs",
        "https://garden.example:444/docs",
        "http://garden.example/docs",
    ] {
        assert!(credentials.for_url(&Url::parse(target).unwrap()).is_empty());
    }
    assert_eq!(
        credentials
            .for_url(&Url::parse("https://garden.example:443/other").unwrap())
            .len(),
        2
    );
}
