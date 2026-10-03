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
