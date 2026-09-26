//! Installing the service: a survey, which is all a preview does, changes
//! nothing; an install writes the binary and the unit and starts the
//! service, and a second run then changes nothing; a download that is not
//! the pinned one, or a tool that fails, stops setup before it writes what
//! depends on it; and a rerun after a step failed reloads and restarts the
//! service on what the failed run wrote.

use super::{
    super::{
        release::Release,
        service::{Step, apply, survey, unit_text},
        tools::Tools,
    },
    support::{Fixture, Home, mode},
};
use crate::cli::failure::Failure;
use maestro_kernel::artifact::Digest;
use std::{
    fs::{self, Permissions},
    os::unix::fs::PermissionsExt as _,
};

/// Every step, as a fresh machine needs them.
const FRESH: [Step; 5] = [
    Step::Install,
    Step::WriteUnit,
    Step::Reload,
    Step::Enable,
    Step::Restart,
];

/// The questions a survey asks the user manager, and nothing else.
const QUESTIONS: [&str; 2] = [
    "systemctl --user is-enabled maestro-qdrant.service",
    "systemctl --user is-active maestro-qdrant.service",
];

/// How setup downloads the fixture's archive: over HTTPS alone, within 30 s
/// to connect and 15 minutes in all, and at most 256 MiB.
const DOWNLOAD: &str = "curl --proto =https --proto-redir =https --tlsv1.2 --fail --silent \
                        --show-error --location --connect-timeout 30 --max-time 900 \
                        --max-filesize 268435456 https://example.org/qdrant-test.tar.gz";

/// How setup unpacks the binary from the archive.
const UNPACK: &str = "tar -xzOf - qdrant";

/// Installs `release` in `home` from scratch, and forgets the calls it took.
fn install(home: &Home, release: &Release<'_>) {
    apply(&FRESH, &home.layout(), release, &home.tools()).unwrap();
    home.forget_calls();
}

#[test]
fn a_survey_of_a_fresh_machine_lists_every_step_and_changes_nothing() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let before = home.files();
    let steps = survey(&home.layout(), &fixture.release(), &home.tools()).unwrap();
    assert_eq!(steps, FRESH);
    assert_eq!(home.files(), before, "a preview writes nothing");
    assert_eq!(home.calls(), QUESTIONS, "and downloads nothing");
}

#[test]
fn an_install_sets_the_service_up_and_a_second_run_changes_nothing() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let (layout, release, tools) = (home.layout(), fixture.release(), home.tools());
    apply(&FRESH, &layout, &release, &tools).unwrap();
    assert_eq!(fs::read(&layout.binary).unwrap(), fixture.binary);
    assert_eq!(mode(&layout.binary), 0o700, "the owner's own program");
    assert_eq!(
        fs::read_to_string(&layout.unit).unwrap(),
        unit_text(&layout, &release).unwrap()
    );
    assert_eq!(
        home.calls(),
        [
            DOWNLOAD,
            UNPACK,
            "systemctl --user daemon-reload",
            "systemctl --user enable maestro-qdrant.service",
            "systemctl --user restart maestro-qdrant.service",
        ]
    );
    let installed = home.files();
    home.forget_calls();
    let steps = survey(&layout, &release, &tools).unwrap();
    assert_eq!(steps, []);
    apply(&steps, &layout, &release, &tools).unwrap();
    assert_eq!(home.files(), installed, "the second run changes nothing");
    assert_eq!(home.calls(), QUESTIONS);
}

#[test]
fn a_digest_mismatch_is_refused_before_anything_is_written() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let served = b"not the pinned archive";
    home.serve(served);
    let before = home.files();
    let refusal = apply(&FRESH, &home.layout(), &fixture.release(), &home.tools()).unwrap_err();
    let message = refusal.to_string();
    assert!(matches!(refusal, Failure::Failed(_)), "{refusal:?}");
    assert!(
        message.contains("https://example.org/qdrant-test.tar.gz")
            && message.contains(&fixture.archive_sha256)
            && message.contains(Digest::of(served).as_str()),
        "names the archive, the pinned digest and the one it has: {message}"
    );
    assert_eq!(home.files(), before, "nothing written");
    assert_eq!(home.calls(), [DOWNLOAD], "nothing unpacked or started");
}

#[test]
fn a_binary_that_is_not_the_pinned_one_is_refused_before_anything_is_written() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let other = Digest::of(b"another qdrant");
    let release = Release {
        binary_sha256: other.as_str(),
        ..fixture.release()
    };
    let before = home.files();
    let refusal = apply(&FRESH, &home.layout(), &release, &home.tools()).unwrap_err();
    let message = refusal.to_string();
    assert!(
        message.contains(other.as_str()) && message.contains(&fixture.binary_sha256),
        "names the pinned digest and the one it has: {message}"
    );
    assert_eq!(home.files(), before, "nothing written");
    assert_eq!(home.calls(), [DOWNLOAD, UNPACK]);
}

#[test]
fn a_unit_edited_by_hand_is_written_again_and_the_service_restarted() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let (layout, release, tools) = (home.layout(), fixture.release(), home.tools());
    install(&home, &release);
    fs::write(&layout.unit, "[Service]\nExecStart=/bin/false\n").unwrap();
    let steps = survey(&layout, &release, &tools).unwrap();
    assert_eq!(steps, [Step::WriteUnit, Step::Reload, Step::Restart]);
    apply(&steps, &layout, &release, &tools).unwrap();
    assert_eq!(
        fs::read_to_string(&layout.unit).unwrap(),
        unit_text(&layout, &release).unwrap()
    );
    assert!(home.calls().ends_with(&[
        "systemctl --user daemon-reload".to_owned(),
        "systemctl --user restart maestro-qdrant.service".to_owned(),
    ]));
}

#[test]
fn a_binary_changed_by_hand_is_installed_again_and_the_service_restarted() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let (layout, release, tools) = (home.layout(), fixture.release(), home.tools());
    install(&home, &release);
    fs::write(&layout.binary, "#!/bin/sh\nexit 1\n").unwrap();
    let steps = survey(&layout, &release, &tools).unwrap();
    assert_eq!(steps, [Step::Install, Step::Restart]);
    apply(&steps, &layout, &release, &tools).unwrap();
    assert_eq!(fs::read(&layout.binary).unwrap(), fixture.binary);
    assert_eq!(mode(&layout.binary), 0o700);
}

#[test]
fn a_binary_that_lost_its_mode_is_installed_again() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let (layout, release, tools) = (home.layout(), fixture.release(), home.tools());
    install(&home, &release);
    fs::set_permissions(&layout.binary, Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        survey(&layout, &release, &tools).unwrap(),
        [Step::Install, Step::Restart]
    );
}

#[test]
fn a_stopped_and_disabled_service_is_enabled_and_started() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let (layout, release, tools) = (home.layout(), fixture.release(), home.tools());
    install(&home, &release);
    home.stop_service();
    let steps = survey(&layout, &release, &tools).unwrap();
    assert_eq!(steps, [Step::Enable, Step::Restart]);
    apply(&steps, &layout, &release, &tools).unwrap();
    assert_eq!(
        home.calls(),
        [
            QUESTIONS[0],
            QUESTIONS[1],
            "systemctl --user enable maestro-qdrant.service",
            "systemctl --user restart maestro-qdrant.service",
        ]
    );
}

#[test]
fn a_failed_systemctl_stops_setup_naming_its_command_and_why() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    home.fail_systemctl("daemon-reload");
    let refusal = apply(&FRESH, &home.layout(), &fixture.release(), &home.tools()).unwrap_err();
    let message = refusal.to_string();
    assert!(matches!(refusal, Failure::Failed(_)), "{refusal:?}");
    assert!(
        message.contains("systemctl --user daemon-reload")
            && message.contains("Failed to connect to bus"),
        "{message}"
    );
    assert_eq!(
        home.calls().last().map(String::as_str),
        Some("systemctl --user daemon-reload"),
        "nothing after the step that failed"
    );
}

#[test]
fn a_rerun_after_a_failed_step_restarts_the_service_on_what_was_written() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let (layout, release, tools) = (home.layout(), fixture.release(), home.tools());
    // A unit edited by hand, written again before the reload fails; then a
    // binary changed by hand, installed again before the restart fails. Each
    // time the service still runs, on what it read before.
    let changes: [(&dyn Fn(), &str); 2] = [
        (
            &|| fs::write(&layout.unit, "[Service]\nExecStart=/bin/false\n").unwrap(),
            "daemon-reload",
        ),
        (
            &|| fs::write(&layout.binary, "#!/bin/sh\nexit 1\n").unwrap(),
            "restart",
        ),
    ];
    install(&home, &release);
    for (change, failing) in changes {
        change();
        home.fail_systemctl(failing);
        let steps = survey(&layout, &release, &tools).unwrap();
        apply(&steps, &layout, &release, &tools).unwrap_err();
        home.heal_systemctl(failing);
        let steps = survey(&layout, &release, &tools).unwrap();
        assert_eq!(steps, [Step::Reload, Step::Restart], "after {failing}");
        apply(&steps, &layout, &release, &tools).unwrap();
        assert_eq!(
            survey(&layout, &release, &tools).unwrap(),
            [],
            "nothing pending once the service restarted, after {failing}"
        );
    }
}

#[test]
fn a_mark_that_cannot_be_cleared_fails_the_restart_naming_it() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let (layout, release, tools) = (home.layout(), fixture.release(), home.tools());
    install(&home, &release);
    // A directory where the mark would be: no file to remove.
    fs::create_dir(&layout.pending).unwrap();
    let refusal = apply(&[Step::Restart], &layout, &release, &tools).unwrap_err();
    assert!(matches!(refusal, Failure::Failed(_)), "{refusal:?}");
    let message = refusal.to_string();
    assert!(
        message.contains(&layout.pending.display().to_string()),
        "{message}"
    );
}

#[test]
fn a_failed_download_names_the_archive_and_why() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    home.refuse_downloads();
    let before = home.files();
    let refusal = apply(&FRESH, &home.layout(), &fixture.release(), &home.tools()).unwrap_err();
    let message = refusal.to_string();
    assert!(
        message.contains("https://example.org/qdrant-test.tar.gz") && message.contains("404"),
        "{message}"
    );
    assert_eq!(home.files(), before);
}

#[test]
fn a_tool_that_cannot_run_is_named() {
    let home = Home::new();
    let fixture = Fixture::new(&home);
    let absent = home.root().join("absent-systemctl");
    let tools = Tools {
        systemctl: absent.clone(),
        ..home.tools()
    };
    let refusal = survey(&home.layout(), &fixture.release(), &tools).unwrap_err();
    assert!(
        refusal.to_string().contains(&absent.display().to_string()),
        "{refusal}"
    );
}
