//! The systemd user manager setup needs: without one, setup is refused
//! before any step, naming the setting that starts one on WSL.

use super::{super::service::user_manager, support::Home};
use crate::cli::failure::Failure;

/// How setup asks whether a user manager runs.
const ASKED: &str = "systemctl --user show-environment";

#[test]
fn a_running_user_manager_lets_setup_go_on() {
    let home = Home::new();
    user_manager(&home.tools()).unwrap();
    assert_eq!(home.calls(), [ASKED]);
}

#[test]
fn no_user_manager_refuses_setup_naming_why_and_the_wsl_setting() {
    let home = Home::new();
    home.fail_systemctl("show-environment");
    let refusal = user_manager(&home.tools()).unwrap_err();
    assert!(matches!(refusal, Failure::Refused(_)), "{refusal:?}");
    let message = refusal.to_string();
    assert!(
        message.contains("Failed to connect to bus")
            && message.contains("[boot]")
            && message.contains("systemd=true"),
        "what systemctl said, and the setting: {message}"
    );
    assert_eq!(home.calls(), [ASKED], "nothing but the question");
}
