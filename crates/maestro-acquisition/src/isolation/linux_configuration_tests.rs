//! Host/configuration validation uses the shared private driver, never a public fallback.
use super::{
    linux::{BootstrapMode, Host, Linux},
    port::{Isolation as _, Refusal},
    test_support::{DONE, Groups, accounting, directory, driver, pin, request},
};
use crate::transport::stream::Accounting;
use maestro_kernel::artifact::Digest;
use std::{env::current_dir, sync::atomic::Ordering};
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

#[test]
fn n17_r1_guard_parent_configuration_and_caps() {
    let parent = directory();
    let io = Groups::new();
    io.observe_environment.store(true, Ordering::Release);
    let observed = parent.join("configuration");
    let script = format!(
        concat!(
            "#!/bin/sh\nread -r config\nprintf '%s' \"$config\" > '{}'\n",
            "printf '%s\\n' '{{\"kind\":\"ready\"}}' '{{\"kind\":\"done\"}}'\n",
        ),
        observed.display()
    );
    let mut adapter = driver(&parent, &io, script.as_bytes());
    let relative_host = Host {
        delegated: adapter.host.delegated.clone(),
        scratch: PathBuf::from("../".repeat(current_dir().unwrap().components().count() - 1))
            .join(adapter.host.scratch.strip_prefix("/").unwrap()),
        bootstrap: pin(Path::new("/bin/true")),
        bootstrap_mode: BootstrapMode::Sealed,
        apparmor_required: false,
    };
    assert!(relative_host.scratch.is_dir());
    assert_eq!(
        fs::metadata(&relative_host.scratch)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert!(matches!(
        Linux::with_io(relative_host, io.clone()),
        Err(Refusal::Configuration)
    ));
    let mut ledger = accounting();
    let mut bounds = ledger.limits().clone();
    bounds.memory_bytes = 800_000.try_into().unwrap();
    bounds.decode.memory_bytes = 500_000.try_into().unwrap();
    ledger.tighten(&bounds);
    assert_eq!(adapter.run(request(&parent), &mut ledger), Ok(vec![]));
    let config: serde_json::Value = serde_json::from_slice(&fs::read(&observed).unwrap()).unwrap();
    assert_eq!(config["memory_bytes"], 500_000);
    assert_eq!(io.child_environment.lock().unwrap().last().unwrap(), b"");
    // Bound the same serialized config, independently of live FD digit counts.
    for descriptor in [7, 1007] {
        let mut config: super::launch::Configuration =
            serde_json::from_slice(&fs::read(&observed).unwrap()).unwrap();
        config.parser_fd = descriptor;
        config.bootstrap_fd = descriptor + 1;
        config.arguments = vec![String::new()];
        let base = serde_json::to_string(&config).unwrap().len();
        config.arguments[0] = "x".repeat(4 * 1024 * 1024 - base);
        assert_eq!(
            super::linux::encode(&config).unwrap().len(),
            4 * 1024 * 1024
        );
        config.arguments[0].push('x');
        assert_eq!(super::linux::encode(&config), Err(Refusal::Configuration));
    }
    adapter.host.bootstrap = pin(Path::new("/bin/sh"));
    adapter.host.bootstrap_mode = BootstrapMode::Installed;
    adapter.host.apparmor_required = true;
    assert!(adapter.run(request(&parent), &mut accounting()).is_err());
    assert_eq!(
        io.child_environment.lock().unwrap().last().unwrap(),
        b"MAESTRO_N17_PROFILE=required\0"
    );
    fs::remove_dir_all(parent).unwrap();
    parent_envelope_winners();
}

/// Every component can independently win the same cumulative wire ceiling.
fn parent_envelope_winners() {
    for winner in ["memory", "decode", "staging"] {
        let parent = directory();
        let io = Groups::new();
        let script = concat!(
            "#!/bin/sh\nread -r config\nprintf '%s%1200s\\n' '{\"kind\":\"ready\"}' ''\n",
            "printf '%s\\n' ",
            "'{\"kind\":\"decode\",\"value\":{\"stage\":\"office\",\"input_bytes\":100,",
            "\"expanded_bytes\":10,\"levels\":0,\"members\":0,\"entities\":0,",
            "\"pixels\":0,\"memory_bytes\":0}}'\n",
            "read -r ack; test \"$ack\" = OK || exit 9\n",
            "printf '{\"kind\":\"data\",\"value\":\"%010d\"}\\n' 0\n",
            "printf '%s\\n' '{\"kind\":\"done\"}'\n",
        );
        let adapter = driver(&parent, &io, script.as_bytes());
        let mut bounds = super::test_support::limits();
        bounds.decode.expanded_bytes = 1000.try_into().unwrap();
        bounds.memory_bytes = 10_000.try_into().unwrap();
        bounds.decode.memory_bytes = 10_000.try_into().unwrap();
        bounds.staging_bytes = 10_000.try_into().unwrap();
        match winner {
            "memory" => bounds.memory_bytes = 1000.try_into().unwrap(),
            "decode" => bounds.decode.memory_bytes = 1000.try_into().unwrap(),
            _ => bounds.staging_bytes = 1000.try_into().unwrap(),
        }
        let mut ledger = Accounting::new(bounds);
        ledger.encoded(100).unwrap();
        assert_eq!(
            adapter.run(request(&parent), &mut ledger),
            Err(Refusal::Output),
            "{winner}"
        );
        let mut bounds = super::test_support::limits();
        bounds.staging_bytes = 10_000.try_into().unwrap();
        let mut ledger = Accounting::new(bounds);
        ledger.encoded(100).unwrap();
        assert_eq!(
            adapter.run(request(&parent), &mut ledger),
            Ok(b"0000000000".to_vec())
        );
        fs::remove_dir_all(parent).unwrap();
    }
}
