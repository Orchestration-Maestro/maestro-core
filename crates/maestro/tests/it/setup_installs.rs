//! `maestro setup` as its users run it: on Linux on x86-64, a preview that
//! changes nothing, an install that refuses a download that is not the
//! pinned archive before it writes anything, and a machine without a systemd
//! user manager refused before any step, with exit 2; on any other platform,
//! the manual steps and exit 2. The install itself, with a small release, is
//! tested in the crate.

use super::support::Home;
use std::path::PathBuf;

/// The service's unit in `home`'s configuration home.
fn unit(home: &Home) -> PathBuf {
    home.root()
        .join("config")
        .join("systemd")
        .join("user")
        .join("maestro-qdrant.service")
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::{
        super::{
            fakes::{Fakes, SERVED},
            support::{Ended, Home, Running},
        },
        unit,
    };
    use maestro_kernel::artifact::Digest;
    use serde_json::json;
    use std::path::PathBuf;

    /// Runs the binary with `arguments` in `home`, the fake tools first on
    /// its `PATH`.
    fn run(home: &Home, fakes: &Fakes, arguments: &[&str]) -> Ended {
        let mut command = home.command(arguments);
        command.env("PATH", fakes.path());
        Running::of(command).finish()
    }

    /// The questions setup asks the user manager before it surveys: whether
    /// one runs; then the survey's, and nothing else.
    const QUESTIONS: [&str; 3] = [
        "systemctl --user show-environment",
        "systemctl --user is-enabled maestro-qdrant.service",
        "systemctl --user is-active maestro-qdrant.service",
    ];

    #[test]
    fn setup_previews_the_service_it_would_install_and_changes_nothing() {
        let home = Home::bare();
        let fakes = Fakes::in_home(&home);
        let qdrant = home.data().join("qdrant");
        let preview = run(&home, &fakes, &["setup"]);
        assert_eq!((preview.code, preview.stderr.as_str()), (Some(0), ""));
        for expected in [
            qdrant.join("bin").join("qdrant").display().to_string(),
            qdrant.join("storage").display().to_string(),
            unit(&home).display().to_string(),
            "127.0.0.1:6333".to_owned(),
            "127.0.0.1:6334".to_owned(),
            "maestro setup --yes".to_owned(),
            "  download https://github.com/qdrant/qdrant/releases/download/v1.19.1/\
             qdrant-x86_64-unknown-linux-gnu.tar.gz, check it"
                .to_owned(),
            "  write the unit\n".to_owned(),
            "  start the service, or restart it on what changed".to_owned(),
        ] {
            assert!(
                preview.stdout.contains(&expected),
                "{expected} is missing from:\n{}",
                preview.stdout
            );
        }
        let json = run(&home, &fakes, &["setup", "--json"]);
        assert_eq!((json.code, json.stderr.as_str()), (Some(0), ""));
        let path = |path: PathBuf| path.display().to_string();
        assert_eq!(
            json.json(),
            json!({
                "schema": "maestro-cli/setup/1",
                "version": "1.19.1",
                "service": "maestro-qdrant.service",
                "binary": path(qdrant.join("bin").join("qdrant")),
                "storage": path(qdrant.join("storage")),
                "snapshots": path(qdrant.join("snapshots")),
                "unit": path(unit(&home)),
                "http": "127.0.0.1:6333",
                "grpc": "127.0.0.1:6334",
                "steps": ["install", "write_unit", "reload", "enable", "restart"],
                "changed": false,
            })
        );
        assert!(!qdrant.exists() && !unit(&home).exists(), "nothing written");
        assert!(
            !home.data().join("kernel.sqlite3").exists(),
            "not even the kernel's database"
        );
        assert_eq!(fakes.calls(), [QUESTIONS, QUESTIONS].concat());
    }

    #[test]
    fn a_machine_without_a_user_manager_is_refused_before_any_step() {
        let home = Home::bare();
        let fakes = Fakes::in_home(&home);
        fakes.without_user_manager();
        for arguments in [&["setup"][..], &["setup", "--yes", "--json"]] {
            let refused = run(&home, &fakes, arguments);
            assert_eq!(refused.code, Some(2), "{arguments:?}: {refused:?}");
            assert_eq!(refused.stdout, "", "{arguments:?}");
            assert!(
                refused.stderr.contains("[boot]") && refused.stderr.contains("systemd=true"),
                "{arguments:?}: {refused:?}"
            );
        }
        assert!(!home.data().join("qdrant").exists() && !unit(&home).exists());
        assert_eq!(fakes.calls(), [QUESTIONS[0], QUESTIONS[0]], "no step taken");
    }

    #[test]
    fn a_download_that_is_not_the_pinned_archive_is_refused_and_nothing_is_written() {
        let home = Home::bare();
        let fakes = Fakes::in_home(&home);
        let refused = run(&home, &fakes, &["setup", "--yes", "--json"]);
        assert_eq!(refused.code, Some(1), "{refused:?}");
        assert_eq!(refused.stdout, "");
        assert!(
            refused
                .stderr
                .contains("eef986e769d4d3e806dd2d546e1b4ecdd416211e54d34b4ed764fac7c58e1085")
                && refused.stderr.contains(Digest::of(SERVED).as_str()),
            "names the pinned digest and the one it got: {refused:?}"
        );
        assert!(!home.data().join("qdrant").exists() && !unit(&home).exists());
        let mut calls = fakes.calls();
        let download = calls.pop().unwrap();
        assert_eq!(calls, QUESTIONS);
        assert!(
            download.starts_with("curl --proto =https ")
                && download.ends_with(
                    " https://github.com/qdrant/qdrant/releases/download/v1.19.1/\
                     qdrant-x86_64-unknown-linux-gnu.tar.gz"
                ),
            "{download}"
        );
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
#[test]
fn setup_elsewhere_prints_the_manual_steps_and_exits_two() {
    let home = Home::bare();
    for arguments in [&["setup"][..], &["setup", "--yes", "--json"]] {
        let refused = home.run(arguments);
        assert_eq!(refused.code, Some(2), "{arguments:?}: {refused:?}");
        assert_eq!(refused.stdout, "", "{arguments:?}");
        assert!(
            refused.stderr.contains("Qdrant 1.19.1") && refused.stderr.contains("maestro doctor"),
            "{arguments:?}: {refused:?}"
        );
    }
    assert!(!home.data().join("qdrant").exists() && !unit(&home).exists());
}
