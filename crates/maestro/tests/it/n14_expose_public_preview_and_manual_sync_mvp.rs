//! Public acquisition CLI registration and refusal boundaries.
use super::support::Home;
use maestro_kernel::acquisition::Handle;
use serde_json::Value;
use std::fs;

#[test]
fn n14_registers_only_preview_sync_and_inspect() {
    let home = Home::new();
    let help = home.run(&["knowledge", "acquire", "--help"]);
    assert_eq!(help.code, Some(0), "{help:?}");
    for operation in ["preview", "sync", "inspect"] {
        assert!(help.stdout.contains(operation), "{help:?}");
    }
    for operation in ["watch", "repair", "withdraw", "browser"] {
        assert!(!help.stdout.contains(operation), "{help:?}");
    }
}

#[test]
fn n14_invalid_manifest_refuses_before_any_start() {
    let home = Home::new();
    let manifest = home.root().join("invalid.json");
    let bindings = home.root().join("bindings.json");
    fs::write(
        &manifest,
        include_bytes!("../../../maestro-acquisition/tests/fixtures/collection.json"),
    )
    .unwrap();
    fs::write(
        &bindings,
        b"{\"schema\":\"maestro-acquisition-bindings/1\",\"resources\":[]}",
    )
    .unwrap();
    for operation in ["preview", "sync"] {
        let result = home.run(&[
            "--json",
            "knowledge",
            "acquire",
            operation,
            "--manifest",
            manifest.to_str().unwrap(),
            "--bindings",
            bindings.to_str().unwrap(),
            "--authority-socket",
            "unused",
            "--authority-uid",
            "65534",
        ]);
        assert_eq!(result.code, Some(2), "{result:?}");
        let output: Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(output["status"], "refused");
        assert_eq!(output["started"], 0);
        assert!(!home.data().join("kernel.sqlite3").exists());
    }
}

#[test]
fn n14_inspect_receipt_and_run_are_real_exclusive_read_operations() {
    let home = Home::new();
    let unknown = Handle::new().to_string();
    for selector in ["--receipt", "--run"] {
        let result = home.run(&[
            "--json",
            "knowledge",
            "acquire",
            "inspect",
            selector,
            &unknown,
        ]);
        assert_eq!(result.code, Some(2), "{result:?}");
        let output: Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(output["status"], "refused");
        assert_eq!(output["started"], 0);
        assert!(
            !home.data().join("kernel.sqlite3").exists(),
            "inspect wrote an empty kernel"
        );
    }
    let result = home.run(&[
        "knowledge",
        "acquire",
        "inspect",
        "--receipt",
        &unknown,
        "--run",
        &unknown,
    ]);
    assert_eq!(result.code, Some(2));
    assert!(result.stdout.is_empty());
    let result = home.run(&["knowledge", "acquire", "sync", "--mode", "full"]);
    assert_eq!(result.code, Some(2));
    assert!(
        result.stderr.contains("unexpected argument '--mode'"),
        "{result:?}"
    );
}
