//! The settings check: the preferences files a session reads, or the one
//! it refuses, named with its key.

use super::{
    super::settings::settings_check,
    support::{Scratch, detail, failure},
};
use crate::settings::Session;
use maestro_settings::USER_FILE;

#[test]
fn settings_check_names_the_files_read_and_what_it_found() {
    let scratch = Scratch::new();
    let session = Session::at(&scratch.config(), None, None, &[]);
    let check = settings_check(&scratch.config(), session);
    let user = scratch.config().join(USER_FILE);
    assert_eq!(check.target, user.display().to_string());
    assert_eq!(
        detail(&check),
        "user file absent (defaults apply); no project file: no working directory to start from"
    );
    scratch.configure(
        USER_FILE,
        "schema = \"maestro-preferences/1\"\ntone = \"brief\"\n",
    );
    let work = scratch.data();
    let session = Session::at(&scratch.config(), Some(&work), None, &[]);
    assert_eq!(
        detail(&settings_check(&scratch.config(), session)),
        "user file read; no project file: no home directory is known: no project file is read"
    );
    let session = Session::at(&scratch.config(), Some(&work), Some(&scratch.data()), &[]);
    assert_eq!(
        detail(&settings_check(&scratch.config(), session)),
        "user file read; no project file found"
    );
}

#[test]
fn settings_check_fails_on_a_refused_file_naming_its_key() {
    let scratch = Scratch::new();
    scratch.configure(
        USER_FILE,
        "schema = \"maestro-preferences/1\"\nsearch.foo = 1\n",
    );
    let session = Session::at(&scratch.config(), None, None, &[]);
    let check = settings_check(&scratch.config(), session);
    let user = scratch.config().join(USER_FILE);
    assert_eq!(
        failure(&check),
        (
            format!("{}: unknown key \"search.foo\"", user.display()).as_str(),
            "fix or remove the key it names; `maestro config explain` shows what each setting \
             accepts"
        )
    );
}
