//! Common defaults have no secret setting; refusals never quote literal values.
use crate::{limits::Limits, settings::defaults::manifest_registry};
use maestro_settings::Registry;

#[test]
fn manifest_default_secret_literals_are_redacted() {
    const SENTINEL: &str = "c60-synthetic-resolved-value-never-output";
    let registry = Registry::built_in().unwrap();
    let neighbour = "schema = 'maestro-preferences/1'\ntone = 'brief'\n";
    assert!(
        manifest_registry(
            &registry,
            &[("defaults".to_owned(), neighbour.to_owned())],
            &Limits::PRODUCTION
        )
        .is_ok()
    );
    for key in ["credential", "env", "keychain", "tone"] {
        let text = format!("schema = 'maestro-preferences/1'\n{key} = '{SENTINEL}'\n");
        let error = manifest_registry(
            &registry,
            &[("defaults".to_owned(), text)],
            &Limits::PRODUCTION,
        )
        .unwrap_err();
        assert!(!format!("{error:?}").contains(SENTINEL));
    }
}
