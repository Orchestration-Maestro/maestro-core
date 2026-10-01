//! Where setup installs, and what it says elsewhere: the pinned release is
//! for Linux on x86-64, the reference workstation's, and every other
//! platform gets the manual steps, a refusal every host tests.

use super::super::release::{QDRANT, Unsupported, release_for};
use std::path::Path;

#[test]
fn the_pinned_release_is_the_one_research_measured() {
    assert_eq!(QDRANT.version, "1.19.1");
    assert_eq!(
        QDRANT.archive,
        "https://github.com/qdrant/qdrant/releases/download/v1.19.1/\
         qdrant-x86_64-unknown-linux-gnu.tar.gz"
    );
    assert_eq!(
        QDRANT.archive_sha256,
        "eef986e769d4d3e806dd2d546e1b4ecdd416211e54d34b4ed764fac7c58e1085"
    );
    assert_eq!(
        QDRANT.binary_sha256,
        "f1823c24376c4a5f2f665d42dc2f52fe4e537e7c2b070b8ae7da575d83327e3a"
    );
}

#[test]
fn setup_installs_on_linux_on_x86_64_only() {
    assert_eq!(release_for("linux", "x86_64"), Ok(QDRANT));
    for (os, arch) in [
        ("macos", "aarch64"),
        ("macos", "x86_64"),
        ("windows", "x86_64"),
        ("linux", "aarch64"),
    ] {
        assert_eq!(
            release_for(os, arch),
            Err(Unsupported {
                os: os.to_owned(),
                arch: arch.to_owned(),
            })
        );
    }
}

#[test]
fn another_platform_gets_the_manual_steps_its_own_way_to_start_at_login() {
    let qdrant = Path::new("/data/maestro").join("qdrant");
    let (storage, snapshots) = (qdrant.join("storage"), qdrant.join("snapshots"));
    let starts = [
        ("macos", "aarch64", "start it at login, as a launchd agent;"),
        (
            "windows",
            "x86_64",
            "start it at login, as a scheduled task;",
        ),
        (
            "linux",
            "aarch64",
            "start it at login, as a systemd user unit",
        ),
        (
            "freebsd",
            "x86_64",
            "start it at login, with the platform's service manager;",
        ),
    ];
    for (os, arch, start) in starts {
        let steps = release_for(os, arch)
            .unwrap_err()
            .manual_steps(&storage, &snapshots);
        for expected in [
            &format!("{os} on {arch}"),
            "Qdrant 1.19.1",
            "https://github.com/qdrant/qdrant/releases/tag/v1.19.1",
            "QDRANT__SERVICE__HOST=127.0.0.1",
            "QDRANT__SERVICE__HTTP_PORT=6333",
            "QDRANT__SERVICE__GRPC_PORT=6334",
            "QDRANT__TELEMETRY_DISABLED=true",
            "--disable-telemetry",
            &format!("QDRANT__STORAGE__STORAGE_PATH={}", storage.display()),
            &format!("QDRANT__STORAGE__SNAPSHOTS_PATH={}", snapshots.display()),
            start,
            "maestro doctor",
        ] {
            assert!(
                steps.contains(expected),
                "{expected} is missing from:\n{steps}"
            );
        }
        let others = starts.iter().filter(|(other, ..)| *other != os);
        for (_, _, other) in others {
            assert!(!steps.contains(other), "{other} is in:\n{steps}");
        }
    }
}
