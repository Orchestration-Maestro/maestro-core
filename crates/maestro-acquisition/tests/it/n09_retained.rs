//! Tightened cumulative envelopes refuse before further effects, even without DATA.
use super::{n07_parse_url_identity_and_denial_precedence::Controls, n09_support::*};
use maestro_acquisition::transport::{http::Failure, stream::Accounting};

#[test]
fn n09_retained_decode_counters_refuse_before_effects() {
    run(async {
        for expected in [
            Failure::ExpandedBytes,
            Failure::ExpansionRatio,
            Failure::Members,
            Failure::Memory,
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(200, "", b"")]);
            let source = policy.policy().sources.first().unwrap();
            let mut accounting = Accounting::new(source.limits.clone());
            accounting.encoded(10).unwrap();
            accounting.decoded(20).unwrap();
            accounting.member().unwrap();
            accounting.member().unwrap();
            let mut tightened = source.limits.clone();
            match expected {
                Failure::ExpandedBytes => tightened.decode.expanded_bytes = 19.try_into().unwrap(),
                Failure::ExpansionRatio => tightened.decode.expansion_ratio = 1.try_into().unwrap(),
                Failure::Members => tightened.decode.members = 1.try_into().unwrap(),
                Failure::Memory => tightened.memory_bytes = 59.try_into().unwrap(),
                _ => panic!("unexpected fixture failure"),
            }
            accounting.tighten(&tightened);
            let result = http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await;
            assert_eq!(result.unwrap_err(), expected);
            assert_eq!(grants.calls.get(), 0, "retained refusal precedes authority");
            assert_eq!(dns.calls.get(), 0, "retained refusal precedes DNS");
            assert_eq!(wire.responses.lock().unwrap().len(), 1);
            assert_eq!(accounting.wire_bytes(), 10);
            assert_eq!(accounting.expanded_bytes(), 20);
            assert_eq!(accounting.members(), 2);
        }
    });
}
