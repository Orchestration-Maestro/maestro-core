//! Graph health preferences never relax runtime adapter admission.
use super::support::Scratch;
use crate::{cli::session, settings::GraphEngine};
use maestro_catalog::settings::NoWorkspaceTrust;

#[test]
fn the_graph_engine_is_none_unless_ladybug_is_selected() {
    let scratch = Scratch::new();
    for (flags, expected) in [
        (vec![], GraphEngine::None),
        (
            vec!["graph.engine=ladybug".to_owned()],
            GraphEngine::Ladybug,
        ),
        (vec!["graph.engine=none".to_owned()], GraphEngine::None),
    ] {
        let session =
            session::health_at(&scratch.config(), None, None, &flags, &NoWorkspaceTrust).unwrap();
        assert_eq!(GraphEngine::from_session(&session).unwrap(), expected);
    }
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
