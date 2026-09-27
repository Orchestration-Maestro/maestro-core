//! Help for explicit projection recovery.

use super::support::Home;

#[test]
fn publish_help_exposes_explicit_republication() {
    let home = Home::bare();
    let result = home.run(&["knowledge", "publish", "--help"]);
    assert_eq!(result.code, Some(0), "{result:?}");
    assert!(result.stdout.contains("--again"), "{result:?}");
}
