//! Dispatch trust-boundary stimuli, independent of namespace availability.
use super::bootstrap_tests::{config, effects, invoke};
use super::{bootstrap::dispatch, port::Refusal};
use std::io;

#[test]
fn n17_bootstrap_unknown_mode_each_arity_refuses_before_effects() {
    let encoded = serde_json::to_string(&config()).unwrap();
    for args in [
        vec!["bogus", encoded.as_str()],
        vec!["bogus", "root", "canary"],
        vec!["bogus", "root", "canary", "loader"],
    ] {
        let fx = effects();
        assert_eq!(
            invoke(&args, b"", false, &fx).0,
            Err(Refusal::Configuration)
        );
        assert!(fx.events.borrow().is_empty());
    }
}
#[test]
fn n17_bootstrap_init_config_limit_exact_and_one_over() {
    let mut encoded = serde_json::to_string(&config()).unwrap();
    encoded.push_str(&" ".repeat(4 * 1024 * 1024 - encoded.len()));
    assert_eq!(encoded.len(), 4 * 1024 * 1024);
    assert_eq!(
        invoke(&["init", &encoded], b"", false, &effects()).0,
        Ok(())
    );
    encoded.push(' ');
    assert!(serde_json::from_str::<serde_json::Value>(&encoded).is_ok());
    let fx = effects();
    assert_eq!(
        invoke(&["init", &encoded], b"", false, &fx).0,
        Err(Refusal::Configuration)
    );
    assert!(fx.events.borrow().is_empty());
}
#[test]
fn n17_bootstrap_probe_extra_arguments_refuse_before_effects() {
    // Negative descriptors cannot name a live inherited FD.
    assert_eq!(invoke(&["probe", "-1"], b"", false, &effects()).0, Ok(()));
    let fx = effects();
    assert_eq!(
        invoke(&["probe", "-1", "extra"], b"", false, &fx).0,
        Err(Refusal::Configuration)
    );
    assert!(fx.events.borrow().is_empty());
}
/// Every attempted write fails, regardless of formatting chunk boundaries.
struct BrokenWriter;
impl io::Write for BrokenWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn n17_bootstrap_probe_and_diagnostics_write_errors_refuse_before_namespaces() {
    let encoded = serde_json::to_vec(&config()).unwrap();
    for probe in [true, false] {
        let fx = effects();
        let args = if probe {
            vec!["probe".to_owned()]
        } else {
            vec![]
        };
        let mut good = Vec::new();
        let mut broken = BrokenWriter;
        let result = if probe {
            dispatch(
                args.into_iter(),
                &mut io::Cursor::new(&encoded),
                (&mut broken, &mut good),
                true,
                &fx,
            )
        } else {
            dispatch(
                args.into_iter(),
                &mut io::Cursor::new(&encoded),
                (&mut good, &mut broken),
                true,
                &fx,
            )
        };
        assert_eq!(result, Err(Refusal::Containment));
        assert_eq!(
            *fx.events.borrow(),
            [if probe { "fds:[]" } else { "fds:[18, 17]" }, "profile"]
        );
    }
}

#[test]
fn n17_bootstrap_profile_requires_exact_installed_or_numeric_scoped_name_and_mode() {
    let encoded = serde_json::to_vec(&config()).unwrap();
    for (profile, accepted) in [
        ("maestro-n17-parser-bootstrap (enforce)", true),
        ("maestro-n17-parser-bootstrap (unconfined)\n", true),
        ("maestro-n17-parser-bootstrap-123-1 (enforce)\n", true),
        ("maestro-n17-parser-bootstrap-123-1 (unconfined)", true),
        ("maestro-n17-parser-bootstrap (complain)", false),
        ("maestro-n17-parser-bootstrap-123-1 (complain)", false),
        ("maestro-n17-parser-bootstrap-lookalike (enforce)", false),
        ("maestro-n17-parser-bootstrap-abc-1 (enforce)", false),
        ("maestro-n17-parser-bootstrap-1-abc (enforce)", false),
        ("maestro-n17-parser-bootstrap-1-2-3 (enforce)", false),
        ("maestro-n17-parser-bootstrap-1 (enforce)", false),
        ("maestro-n17-parser-bootstrap--1 (enforce)", false),
        ("maestro-n17-parser-bootstrap-1- (enforce)", false),
        ("maestro-n17-parser-bootstrap-١-1 (enforce)", false),
        ("maestro-n17-parser-bootstrap", false),
        ("maestro-n17-parser-bootstrap ", false),
        ("maestro-n17-parser-bootstrap (enforce) extra", false),
        ("maestro-n17-parser-bootstrap (enforce) ", false),
        (" maestro-n17-parser-bootstrap (enforce)", false),
    ] {
        let mut fx = effects();
        fx.profile = profile.into();
        let result = invoke(&[], &encoded, true, &fx).0;
        assert_eq!(
            result,
            if accepted {
                Ok(())
            } else {
                Err(Refusal::Unsupported)
            },
            "profile {profile:?}"
        );
        assert_eq!(
            fx.events
                .borrow()
                .iter()
                .any(|event| event.starts_with("unshare:")),
            accepted,
            "profile refusal must precede namespaces"
        );
    }
}
