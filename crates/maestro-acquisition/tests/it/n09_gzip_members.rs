//! RFC 1952 member accounting is shared with every decode stage.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls, n09_decode::compressed, n09_support::*,
};
use maestro_acquisition::transport::{http::Failure, stream::Accounting};

#[test]
fn n09_concatenated_gzip_returns_all_members() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let mut bytes = compressed(true, b"first");
        bytes.extend(compressed(true, b"second"));
        let wire = Wire::new(vec![response(200, "Content-Encoding: gzip\r\n", &bytes)]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let result = http(&policy, &controls, &grants, &dns, &wire)
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await
            .unwrap();
        assert_eq!(result.body, b"firstsecond");
        assert_eq!(accounting.expanded_bytes(), 11);
        assert_eq!(accounting.members(), 2);
    });
}
#[test]
fn n09_empty_gzip_member_consumes_cumulative_member_budget() {
    run(async {
        for member_limit in [1, 2] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let mut bytes = compressed(true, b"");
            bytes.extend(compressed(true, b"second"));
            let wire = Wire::new(vec![response(200, "Content-Encoding: gzip\r\n", &bytes)]);
            let mut limits = policy.policy().sources[0].limits.clone();
            limits.decode.members = member_limit.try_into().unwrap();
            let mut accounting = Accounting::new(limits);
            let result = http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await;
            if member_limit == 1 {
                assert_eq!(result.unwrap_err(), Failure::Members);
            } else {
                assert_eq!(result.unwrap().body, b"second");
                assert_eq!(accounting.members(), 2);
            }
        }
    });
}
#[test]
fn n09_gzip_second_member_crc_and_trailing_garbage_refuse() {
    run(async {
        let mut second = compressed(true, b"second");
        let crc = second.len() - 8;
        *second.get_mut(crc).unwrap() ^= 1;
        for tail in [second, b"garbage".to_vec(), vec![0, 0], vec![0x1f]] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let mut bytes = compressed(true, b"first");
            bytes.extend(tail);
            let wire = Wire::new(vec![response(200, "Content-Encoding: gzip\r\n", &bytes)]);
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                Failure::Content
            );
        }
    });
}
#[test]
fn n09_ratio_budget_spans_concatenated_gzip_members() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let mut bytes = compressed(true, b"first");
        bytes.extend(compressed(true, &vec![b'x'; 1000]));
        let wire = Wire::new(vec![response(200, "Content-Encoding: gzip\r\n", &bytes)]);
        let mut limits = policy.policy().sources[0].limits.clone();
        limits.decode.expansion_ratio = 2.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::ExpansionRatio
        );
    });
}
