//! Real host metadata regression fixtures.
use super::authority_host::{Host, binding, checked, probe, protected, qualified, separated};
use maestro_test_scratch::scratch_directory;
use rustix::process::geteuid;
use std::{
    fs,
    os::unix::{
        fs::{PermissionsExt as _, symlink},
        net::UnixListener,
    },
    path::{Path, PathBuf},
};

/// Only pure metadata/identity tests; never runs a launcher or qualifies live grants.
pub(super) struct Scratch(pub(super) PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
pub(super) fn fixture() -> (Scratch, Host, PathBuf) {
    let scratch = Scratch(scratch_directory().unwrap());
    let store = scratch.0.join("store");
    fs::create_dir(&store).unwrap();
    fs::set_permissions(&store, fs::Permissions::from_mode(0o700)).unwrap();
    let launcher = scratch.0.join("launcher");
    fs::write(&launcher, "synthetic metadata only").unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        scratch.0.join("maestro"),
        "synthetic probe bytes, never executable",
    )
    .unwrap();
    fs::set_permissions(scratch.0.join("maestro"), fs::Permissions::from_mode(0o600)).unwrap();
    let host = Host {
        store,
        socket: scratch.0.join("socket"),
        owner_uid: geteuid().as_raw(),
        pipeline_uid: 65534,
        connector_uid: 65533,
        launcher,
    };
    let config = scratch.0.join("config.json");
    fs::write(&config, serde_json::to_vec(&host).unwrap()).unwrap();
    (scratch, host, config)
}
#[test]
fn n05_host_identity_separation_has_no_root_or_same_user_fallback() {
    let (_scratch, mut host, _config) = fixture();
    assert!(separated(&host, host.owner_uid).is_ok());
    let original = host.owner_uid;
    for identities in [
        [0, 65534, 65533],
        [original, 0, 65533],
        [original, 65534, 0],
        [original, original, 65533],
        [original, 65534, original],
        [original, 65534, 65534],
        [original + 1, 65534, 65533],
    ] {
        host.owner_uid = identities[0];
        host.pipeline_uid = identities[1];
        host.connector_uid = identities[2];
        assert!(separated(&host, original).is_err());
    }
}
#[test]
fn n05_host_metadata_and_configuration_are_strict_before_launch() {
    let (scratch, host, config) = fixture();
    assert!(checked(&config).is_ok());
    for mode in [0o755, 0o770, 0o1700] {
        fs::set_permissions(&host.store, fs::Permissions::from_mode(mode)).unwrap();
        assert!(checked(&config).is_err());
    }
    fs::set_permissions(&host.store, fs::Permissions::from_mode(0o700)).unwrap();
    for mode in [0o600, 0o720] {
        fs::set_permissions(&host.launcher, fs::Permissions::from_mode(mode)).unwrap();
        assert!(checked(&config).is_err());
    }
    fs::set_permissions(&host.launcher, fs::Permissions::from_mode(0o700)).unwrap();
    let link = scratch.0.join("link");
    symlink(&host.store, &link).unwrap();
    assert!(protected(&link, host.owner_uid).is_err());
    assert!(protected(&host.store, host.owner_uid + 1).is_err());
    assert!(protected(Path::new("relative"), host.owner_uid).is_err());
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(protected(&host.store, host.owner_uid).is_err());
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let original = fs::read(&config).unwrap();
    let mut oversized = original.clone();
    oversized.resize(16_385, b' ');
    fs::write(&config, oversized).unwrap();
    assert!(checked(&config).is_err());
    fs::write(&config, original).unwrap();
    let mut value = serde_json::to_value(&host).unwrap();
    value["model_approval"] = true.into();
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(checked(&config).is_err());
    fs::write(&config, "{\"owner_uid\":1000,\"owner_uid\":1000}").unwrap();
    assert!(checked(&config).is_err());
    // Being writable by the actual owner is explicitly not a passing separation probe.
    fs::write(host.store.join("probe-witness"), "authority-probe").unwrap();
    let _listener = UnixListener::bind(&host.socket).unwrap();
    assert!(probe(&host.store, &host.socket).is_err());
}
#[test]
fn n05_receipts_bind_current_host_metadata_and_protected_bytes() {
    let (_scratch, mut host, _config) = fixture();
    let receipt = host.store.join("qualification");
    assert!(qualified(&host).is_err());
    fs::write(&receipt, binding(&host).unwrap()).unwrap();
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600)).unwrap();
    // Tests only the receipt validator; never starts a server with fabricated evidence.
    assert!(qualified(&host).is_ok());
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(qualified(&host).is_err());
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600)).unwrap();
    host.connector_uid = 65532;
    assert!(qualified(&host).is_err());
    host.connector_uid = 65533;
    fs::write(&host.launcher, "substituted launcher").unwrap();
    assert!(qualified(&host).is_err());
    fs::write(&receipt, "forged").unwrap();
    assert!(qualified(&host).is_err());
    fs::remove_file(&receipt).unwrap();
    symlink(&host.launcher, &receipt).unwrap();
    assert!(qualified(&host).is_err());
}
