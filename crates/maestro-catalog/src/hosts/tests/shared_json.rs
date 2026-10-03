//! Entry ownership never adopts matching user entries or overwrites edited owned entries.
use super::super::shared_json::edit;
use serde_json::json;

#[test]
fn hosts_shared_json_preserves_unrelated_and_refuses_edited_ownership() {
    let registration = json!({"type":"stdio", "command":"maestro", "args":["mcp"], "tools":["*"]});
    let original = br#"{"mcpServers":{"user":{"command":"user"}},"other":true}"#;
    let inserted = edit(Some(original), &registration, false, false).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&inserted).unwrap();
    assert_eq!(value["mcpServers"]["user"], json!({"command":"user"}));
    assert_eq!(value["other"], true);
    assert_eq!(
        edit(Some(&inserted), &registration, true, false).unwrap(),
        inserted
    );
    assert!(edit(Some(&inserted), &registration, false, false).is_err());
    assert_eq!(
        edit(Some(&inserted), &registration, false, true).unwrap(),
        inserted
    );
    let edited = inserted
        .windows(7)
        .position(|bytes| bytes == b"maestro")
        .unwrap();
    let mut changed = inserted.clone();
    changed[edited] = b'M';
    assert!(edit(Some(&changed), &registration, true, false).is_err());
    assert!(edit(Some(&changed), &registration, true, true).is_err());
    let removed = edit(Some(&inserted), &registration, true, true).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&removed).unwrap(),
        serde_json::from_slice::<serde_json::Value>(original).unwrap()
    );
    assert_eq!(
        edit(Some(&removed), &registration, false, true).unwrap(),
        removed
    );
    assert!(edit(Some(original), &registration, true, false).is_err());
}

#[test]
fn hosts_shared_json_strict_shape_neighbours() {
    let entry = json!({"command":"maestro"});
    for invalid in [
        b"[]".as_slice(),
        br#"{"mcpServers":[]}"#,
        br#"{"mcpServers":{},"mcpServers":{}}"#,
        b"invalid",
    ] {
        assert!(edit(Some(invalid), &entry, false, false).is_err());
    }
    let valid = edit(None, &entry, false, false).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&valid).unwrap()["mcpServers"]["maestro"],
        entry
    );
}

#[test]
fn hosts_shared_json_insert_remove_preserve_raw_neighbours() {
    let entry = json!({"command":"maestro"});
    for original in [
        concat!(
            r#"{ "mcpServers" : { "user" : "#,
            r#"{"escaped":"\u0075","number":1.2300e+02} }, "#,
            r#""other" : 9007199254740993 }"#,
        )
        .as_bytes(),
        br#"{ "other" : "\u0075", "number" : 1.2300e+02 }"#,
        b"{ \n }",
    ] {
        let inserted = edit(Some(original), &entry, false, false).unwrap();
        let removed = edit(Some(&inserted), &entry, true, true).unwrap();
        for neighbour in [
            br#""other" : "\u0075""#.as_slice(),
            br#""user" : {"escaped":"\u0075","number":1.2300e+02}"#,
            b"9007199254740993",
            b"1.2300e+02",
        ] {
            if original
                .windows(neighbour.len())
                .any(|bytes| bytes == neighbour)
            {
                assert!(
                    inserted
                        .windows(neighbour.len())
                        .any(|bytes| bytes == neighbour)
                );
                assert!(
                    removed
                        .windows(neighbour.len())
                        .any(|bytes| bytes == neighbour)
                );
            }
        }
        serde_json::from_slice::<serde_json::Value>(&removed).unwrap();
    }
}

#[test]
fn hosts_shared_json_depth_boundary() {
    let entry = json!({"command":"maestro"});
    for (levels, accepted) in [(31, true), (32, false)] {
        let input = format!(
            "{{\"other\":{}0{}}}",
            "[".repeat(levels),
            "]".repeat(levels)
        );
        assert_eq!(
            edit(Some(input.as_bytes()), &entry, false, false).is_ok(),
            accepted
        );
    }
}

#[test]
fn hosts_shared_json_owned_removal_requires_server_table() {
    for bytes in [b"{}".as_slice(), b"{\"other\":true}"] {
        assert!(edit(Some(bytes), &json!({"command":"maestro"}), true, true).is_err());
    }
}

#[test]
fn hosts_shared_json_splices_all_owned_member_positions() {
    let entry = json!({"command":"maestro"});
    let raw = br#""user" : { "quote" : "\"},:[\\\u0075", "n" : 1.2300e+02 }"#;
    for members in [
        format!(
            r#""maestro":{entry}, {}, "tail":true"#,
            String::from_utf8_lossy(raw)
        ),
        format!(
            r#"{}, "maestro":{entry}, "tail":true"#,
            String::from_utf8_lossy(raw)
        ),
        format!(
            r#"{}, "tail":true, "maestro":{entry}"#,
            String::from_utf8_lossy(raw)
        ),
        format!(r#""mae\u0073tro":{entry}"#),
    ] {
        let input = format!("{{\"mcp\\u0053ervers\":{{{members}}}}}");
        let removed = edit(Some(input.as_bytes()), &entry, true, true).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&removed).unwrap();
        assert!(document["mcpServers"].get("maestro").is_none());
        if input
            .as_bytes()
            .windows(raw.len())
            .any(|bytes| bytes == raw)
        {
            assert!(removed.windows(raw.len()).any(|bytes| bytes == raw));
        }
    }
    let bytes = br#"{ "mcpServers" : { "user" : "\u0075" }  }"#;
    let inserted = edit(Some(bytes), &entry, false, false).unwrap();
    assert_eq!(edit(Some(&inserted), &entry, true, true).unwrap(), bytes);
}
