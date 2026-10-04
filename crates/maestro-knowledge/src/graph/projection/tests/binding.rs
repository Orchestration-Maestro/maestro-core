//! Admission comparisons independent of native files or a warm handle registry.
use super::contract;
use crate::graph::projection::{EngineSettings, InputMismatchKind, ProjectionError, binding};
use maestro_kernel::artifact::Digest;

#[test]
fn matching_pins_admit_and_changed_settings_or_complete_lock_refuse() {
    let pins = contract::pins();
    let admitted = EngineSettings::new(
        16 * 1024 * 1024,
        64 * 1024 * 1024,
        1,
        Digest::of(b"frozen-lock"),
    )
    .unwrap();
    binding::admitted(&pins, &admitted).unwrap();
    for (name, kind, settings) in [
        (
            "changed settings",
            InputMismatchKind::Settings,
            EngineSettings::new(
                32 * 1024 * 1024,
                64 * 1024 * 1024,
                1,
                admitted.frozen_lock().clone(),
            )
            .unwrap(),
        ),
        (
            "changed complete lock",
            InputMismatchKind::Lock,
            EngineSettings::new(
                16 * 1024 * 1024,
                64 * 1024 * 1024,
                1,
                Digest::of(b"other admitted source"),
            )
            .unwrap(),
        ),
    ] {
        let error = binding::admitted(&pins, &settings).expect_err(name);
        assert_eq!(error, ProjectionError::InputMismatch(kind));
        assert!(
            error
                .to_string()
                .contains("maestro knowledge graph rebuild"),
            "{name}: {error}"
        );
    }
}

#[test]
fn malformed_and_unknown_input_pins_refuse_before_admission() {
    for (field, value) in [
        (0, ""),
        (0, "xyzzy"),
        (1, "unknown/1"),
        (2, ""),
        (3, "xyzzy"),
    ] {
        let mut pins = contract::pins();
        pins[field] = value.into();
        assert!(
            binding::validate(&pins)
                .unwrap_err()
                .contains("maestro knowledge graph rebuild")
        );
    }
}
