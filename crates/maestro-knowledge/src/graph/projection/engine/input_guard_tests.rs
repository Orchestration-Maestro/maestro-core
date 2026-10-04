//! Producer preflight forwards all durable pins before reserving native storage.
use super::public_fixture::{Fixture, now};
use crate::graph::projection::{InputMismatchKind, ProjectionError};
use maestro_kernel::artifact::Digest;
use std::{collections::BTreeSet, fs};

#[test]
fn producer_input_mismatch_refuses_before_reservation() {
    let fixture = Fixture::new();
    let before: BTreeSet<_> = fs::read_dir(&fixture.native.path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    for (index, kind) in [
        InputMismatchKind::Settings,
        InputMismatchKind::Lock,
        InputMismatchKind::Resolution,
    ]
    .into_iter()
    .enumerate()
    {
        let mut build = fixture.build.clone();
        match index {
            0 => build.settings_identity = Digest::of(b"changed settings"),
            1 => build.frozen_lock = Digest::of(b"changed lock"),
            _ => build.resolution_id = Digest::of(b"missing resolution"),
        }
        assert_eq!(
            fixture
                .factory()
                .producer(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    build,
                    &|| now(0)
                )
                .unwrap_err(),
            ProjectionError::InputMismatch(kind)
        );
        let after: BTreeSet<_> = fs::read_dir(&fixture.native.path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            before, after,
            "no reservation or native file before refusal"
        );
        assert!(
            fixture
                .authority
                .database
                .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
                .unwrap()
                .is_none()
        );
    }
}
