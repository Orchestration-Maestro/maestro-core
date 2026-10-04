//! N05 authority command refusals; real identity probes are explicitly opt-in.
use super::support::Home;
use std::fs;

#[test]
fn n05_missing_qualification_refuses_without_creating_a_store() {
    let home = Home::bare();
    let config = home.root().join("authority.json");
    fs::write(
        &config,
        r#"{"store":"missing","socket":"missing","owner_uid":1000,
        "pipeline_uid":65534,"connector_uid":65533,"launcher":"missing"}"#,
    )
    .unwrap();
    let ended = home.run(&["authority", "serve", "--config", config.to_str().unwrap()]);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert!(ended.stderr.contains("authority unqualified"), "{ended:?}");
    assert!(!home.root().join("missing").exists());
}

#[test]
fn n05_generic_yes_and_manifest_approvals_do_not_authenticate() {
    let home = Home::bare();
    for arguments in [
        vec!["authority", "grant", "--yes"],
        vec!["authority", "grant", "--approval", "model-approved"],
    ] {
        let ended = home.run(&arguments);
        assert_eq!(ended.code, Some(2), "{ended:?}");
        assert!(!home.data().join("authority.sqlite3").exists());
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn n05_unqualified_platform_refuses_live_authority() {
    let home = Home::bare();
    let ended = home.run(&["authority", "qualify", "--config", "absent.json"]);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert!(ended.stderr.contains("authority unqualified"), "{ended:?}");
}
