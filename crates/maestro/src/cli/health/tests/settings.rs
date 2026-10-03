//! The settings check: the preferences files a session reads, or the one
//! it refuses, named with its key.

use super::{
    super::settings::settings_check,
    support::{Scratch, detail},
};
use crate::cli::session;
use maestro_settings::USER_FILE;
use std::fs;

#[test]
fn settings_check_names_the_files_read_and_what_it_found() {
    let scratch = Scratch::new();
    let session = session::at(&scratch.config(), None, None, &[]).unwrap();
    let check = settings_check(&scratch.config(), Ok(&session));
    let user = scratch.config().join(USER_FILE);
    assert_eq!(check.target, user.display().to_string());
    assert_eq!(
        detail(&check),
        "user file absent (defaults apply); no project file found"
    );
    scratch.configure(
        USER_FILE,
        "schema = \"maestro-preferences/1\"\ntone = \"brief\"\n",
    );
    let work = scratch.data();
    let session = session::at(&scratch.config(), Some(&work), None, &[]).unwrap();
    assert!(
        detail(&settings_check(&scratch.config(), Ok(&session)))
            .contains("outside home without workspace trust")
    );
    let session = session::at(&scratch.config(), Some(&work), Some(&scratch.data()), &[]).unwrap();
    assert_eq!(
        detail(&settings_check(&scratch.config(), Ok(&session))),
        "user file read; no project file found"
    );
}

#[test]
fn startup_refuses_a_file_naming_its_key_before_doctor() {
    let scratch = Scratch::new();
    scratch.configure(
        USER_FILE,
        "schema = \"maestro-preferences/1\"\nsearch.foo = 1\n",
    );
    let failure = session::at(&scratch.config(), None, None, &[]).unwrap_err();
    let user = scratch.config().join(USER_FILE);
    assert_eq!(
        failure.to_string(),
        format!("{}: unknown key \"search.foo\"", user.display())
    );
}

#[test]
fn doctor_settings_reports_the_original_snapshot_after_preferences_change_or_removal() {
    let scratch = Scratch::new();
    scratch.configure(
        USER_FILE,
        "schema = 'maestro-preferences/1'\ntone = 'brief'\n",
    );
    let session = session::at(&scratch.config(), None, None, &[]).unwrap();
    for remove in [false, true] {
        if remove {
            fs::remove_file(scratch.config().join(USER_FILE)).unwrap();
        } else {
            scratch.configure(
                USER_FILE,
                "schema = 'maestro-preferences/1'\ntone = 'detailed'\n",
            );
        }
        let check = settings_check(&scratch.config(), Ok(&session));
        assert_eq!(detail(&check), "user file read; no project file found");
        assert_eq!(session.resolved().text("tone"), Some("brief"));
    }
}
