//! Host/configuration validation uses the shared private driver, never a public fallback.
use super::{
    linux::{BootstrapMode, Host, Linux},
    port::{Isolation as _, Refusal},
    test_support::{DONE, Groups, accounting, directory, driver, pin, request},
};
use maestro_kernel::artifact::Digest;
use std::{
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};
#[test]
fn n17_linux_host_validation_explicit_posture_and_config_size() {
    let parent = directory();
    let io = Groups::new();
    let mut driver = driver(&parent, &io, DONE);
    let mut launch = request(&parent);
    launch.arguments.push("x".repeat(4 * 1024 * 1024));
    io.calls.lock().unwrap().clear();
    assert_eq!(
        driver.run(launch, &mut accounting()),
        Err(Refusal::Configuration)
    );
    assert!(
        io.calls.lock().unwrap().is_empty(),
        "oversized barrier must refuse before worker create"
    );
    driver.host.bootstrap.digest = Digest::of(b"wrong bootstrap");
    assert_eq!(
        driver.run(request(&parent), &mut accounting()),
        Err(Refusal::LaunchPin)
    );
    assert!(io.calls.lock().unwrap().is_empty());
    for mode in [None, Some(""), Some("automatic"), Some("Installed")] {
        assert_eq!(BootstrapMode::parse(mode), Err(Refusal::Configuration));
    }
    assert_eq!(
        BootstrapMode::parse(Some("sealed")),
        Ok(BootstrapMode::Sealed)
    );
    assert_eq!(
        BootstrapMode::parse(Some("installed")),
        Ok(BootstrapMode::Installed)
    );
    for (scratch, mode, profile) in [
        (parent.join("missing"), BootstrapMode::Sealed, false),
        (PathBuf::from("relative"), BootstrapMode::Sealed, false),
        (parent.join("runs"), BootstrapMode::Sealed, true),
    ] {
        assert!(matches!(
            Linux::with_io(
                Host {
                    delegated: PathBuf::from("/sys/fs/cgroup/n17.service"),
                    scratch,
                    bootstrap: pin(Path::new("/bin/true")),
                    bootstrap_mode: mode,
                    apparmor_required: profile
                },
                io.clone()
            ),
            Err(Refusal::Configuration)
        ));
    }
    let bad = parent.join("bad-mode");
    fs::create_dir(&bad).unwrap();
    fs::set_permissions(&bad, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        Linux::with_io(
            Host {
                delegated: PathBuf::from("/sys/fs/cgroup/n17.service"),
                scratch: bad,
                bootstrap: pin(Path::new("/bin/true")),
                bootstrap_mode: BootstrapMode::Installed,
                apparmor_required: true
            },
            io
        ),
        Err(Refusal::Configuration)
    ));
    fs::remove_dir_all(parent).unwrap();
}
