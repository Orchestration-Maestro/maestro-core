//! Cumulative decoder guards, independently selected synthetic bombs.
use super::{n07_parse_url_identity_and_denial_precedence::Controls, n09_support::*};
use flate2::{
    Compression,
    write::{GzEncoder, ZlibEncoder},
};
use maestro_acquisition::transport::{http::Failure, stream::Accounting};
use std::io::Write as _;

/// Compress fixture bytes with the same wire formats, not the transport decoder.
pub(super) fn compressed(gzip: bool, bytes: &[u8]) -> Vec<u8> {
    if gzip {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    } else {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }
}
#[test]
fn n09_gzip_deflate_and_stacked_decoding() {
    run(async {
        for encoding in ["gzip", "deflate", "gzip, deflate"] {
            let bytes = compressed(encoding == "gzip", b"synthetic document");
            let bytes = if encoding.contains(',') {
                compressed(false, &compressed(true, b"synthetic document"))
            } else {
                bytes
            };
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(
                200,
                &format!("Content-Encoding: {encoding}\r\n"),
                &bytes,
            )]);
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            let result = http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap();
            assert_eq!(result.body, b"synthetic document");
            assert_eq!(accounting.wire_bytes(), bytes.len() as u64);
            assert!(accounting.expanded_bytes() >= 18);
        }
    });
}
#[test]
fn n09_ratio_memory_and_nesting_guards() {
    run(async {
        for (guard, expected) in [
            ("ratio", Failure::ExpansionRatio),
            ("memory", Failure::Memory),
            ("nesting", Failure::Nesting),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let bytes = compressed(false, &compressed(true, &vec![b'x'; 1000]));
            let wire = Wire::new(vec![response(
                200,
                "Content-Encoding: gzip, deflate\r\n",
                &bytes,
            )]);
            let mut limits = policy.policy().sources[0].limits.clone();
            match guard {
                "ratio" => limits.decode.expansion_ratio = 1.try_into().unwrap(),
                "memory" => limits.decode.memory_bytes = 1.try_into().unwrap(),
                _ => limits.decode.nested_levels = 1.try_into().unwrap(),
            }
            let mut accounting = Accounting::new(limits);
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                expected
            );
        }
    });
}
#[test]
fn n09_cumulative_redirect_bodies_and_reused_accounting() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![
            response(302, "Location: /docs/final\r\n", b"123"),
            response(200, "", b"456"),
        ]);
        let mut limits = policy.policy().sources[0].limits.clone();
        limits.wire_bytes = 5.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::EncodedBytes
        );
    });
}
#[test]
fn n09_truncated_compressed_and_http_bodies() {
    run(async {
        let compressed = compressed(true, b"fixture");
        for bytes in [
            response(
                200,
                "Content-Encoding: gzip\r\n",
                compressed.get(..compressed.len() - 2).unwrap(),
            ),
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort".to_vec(),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![bytes]);
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            let result = http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await;
            assert!(matches!(result, Err(Failure::Content | Failure::Partial)));
        }
    });
}

#[test]
fn n09_codec_workspace_is_charged_before_allocation() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(
            200,
            "Content-Encoding: gzip\r\n",
            &compressed(true, b"fixture"),
        )]);
        let mut limits = policy.policy().sources[0].limits.clone();
        limits.decode.memory_bytes = 100_000.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Memory
        );
        assert_eq!(accounting.members(), 0);
    });
}

#[test]
fn n09_per_step_ratio_cannot_borrow_redirect_padding() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let bytes = compressed(true, &vec![b'x'; 5000]);
        assert!(5000 > bytes.len() * 100);
        let wire = Wire::new(vec![
            response(302, "Location: /docs/final\r\n", &vec![b'p'; 1000]),
            response(200, "Content-Encoding: gzip\r\n", &bytes),
        ]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::ExpansionRatio
        );
    });
}

#[test]
fn n09_unknown_encoding_cannot_decode_valid_deflate_bytes() {
    run(async {
        for encoding in ["br", "gzip, br"] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let payload = if encoding.contains(',') {
                compressed(true, b"fixture")
            } else {
                b"fixture".to_vec()
            };
            let wire = Wire::new(vec![response(
                200,
                &format!("Content-Encoding: {encoding}\r\n"),
                &compressed(false, &payload),
            )]);
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
fn n09_deflate_trailing_gzip_member_is_not_accepted() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let mut bytes = compressed(false, b"first");
        bytes.extend(compressed(true, b"second"));
        let wire = Wire::new(vec![response(200, "Content-Encoding: deflate\r\n", &bytes)]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Content
        );
    });
}
