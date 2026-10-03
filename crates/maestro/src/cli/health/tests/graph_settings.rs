//! Graph health preferences never relax runtime adapter admission.
use super::support::Scratch;
use crate::{cli::session, settings::GraphEngine};
use maestro_catalog::{
    policy::workspace::{CheckedTrust, TrustBoundaries},
    settings::NoWorkspaceTrust,
};

#[test]
fn health_preferences_ignore_unadmitted_authoring_locks_without_relaxing_runtime() {
    use std::fs;
    let scratch = Scratch::new();
    let boundaries = TrustBoundaries::new(&scratch.data(), &[]).unwrap();
    let trust = CheckedTrust::new(&NoWorkspaceTrust, &boundaries);
    let workspace = scratch.data().join("workspace");
    let directory = workspace.join(".maestro");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("authoring.lock.json"), b"invalid").unwrap();
    let health = session::health_at(
        &scratch.config(),
        Some(&workspace),
        Some(&scratch.data()),
        &[],
        &trust,
    )
    .unwrap();
    assert_eq!(
        GraphEngine::from_session(&health).unwrap(),
        GraphEngine::None
    );
    assert!(
        session::at(
            &scratch.config(),
            Some(&workspace),
            Some(&scratch.data()),
            &[]
        )
        .is_err()
    );
    assert_eq!(
        fs::read(directory.join("authoring.lock.json")).unwrap(),
        b"invalid"
    );
    assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
}

#[test]
fn the_graph_engine_is_none_unless_ladybug_is_selected() {
    let scratch = Scratch::new();
    let boundaries = TrustBoundaries::new(&scratch.data(), &[]).unwrap();
    let trust = CheckedTrust::new(&NoWorkspaceTrust, &boundaries);
    for (flags, expected) in [
        (vec![], GraphEngine::None),
        (
            vec!["graph.engine=ladybug".to_owned()],
            GraphEngine::Ladybug,
        ),
        (vec!["graph.engine=none".to_owned()], GraphEngine::None),
    ] {
        let session = session::health_at(&scratch.config(), None, None, &flags, &trust).unwrap();
        assert_eq!(GraphEngine::from_session(&session).unwrap(), expected);
    }
    #[cfg(not(feature = "engine"))]
    assert_eq!(
        session::at(
            &scratch.config(),
            None,
            None,
            &["graph.engine=ladybug".to_owned()]
        )
        .unwrap_err()
        .to_string(),
        "backend adapter is not compiled into this build",
    );
}
