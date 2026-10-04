//! Engine-selected setup dispatch; filesystem preparation remains default-testable.
use super::graph::{DIRECTORY, GraphSetup, disabled, prepare};
use crate::{failure::Failure, settings::GraphEngine};
use maestro_kernel::paths::{self, Environment};

/// Resolve the application's data directory and prepare only its selected graph child.
pub(super) fn run(
    environment: &Environment,
    engine: GraphEngine,
    yes: bool,
) -> Result<GraphSetup, Failure> {
    if engine == GraphEngine::None {
        return Ok(disabled());
    }
    let data = paths::data_dir(environment).map_err(|error| Failure::failed_by(&error))?;
    prepare(&data.join(DIRECTORY), yes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_test_scratch::scratch_directory;
    use std::fs;

    #[test]
    fn setup_graph_engine_dispatch_resolves_the_data_directory_and_prepares_only_its_child() {
        let scratch = scratch_directory().unwrap();
        let mut environment = Environment::default();
        environment.xdg_data_home = Some(scratch.clone().into_os_string());
        let graph = scratch.join("maestro").join(DIRECTORY);
        let off = run(&environment, GraphEngine::None, true).unwrap();
        assert_eq!(
            (off.engine, off.action, off.changed),
            ("none", "disabled", false)
        );
        assert_eq!(off.directory, None);
        assert_eq!(fs::read_dir(&scratch).unwrap().count(), 0);
        let preview = run(&environment, GraphEngine::Ladybug, false).unwrap();
        assert_eq!(preview.directory, Some(graph.display().to_string()));
        assert_eq!(
            (preview.action, preview.changed),
            ("create_directory", false)
        );
        assert_eq!(preview.missing_guards, [".access.guard", ".writer.guard"]);
        assert!(!graph.exists());
        let applied = run(&environment, GraphEngine::Ladybug, true).unwrap();
        assert_eq!(applied.directory, Some(graph.display().to_string()));
        assert_eq!(
            (applied.action, applied.changed),
            ("create_directory", true)
        );
        assert!(graph.join(".access.guard").is_file());
        assert!(graph.join(".writer.guard").is_file());
        let again = run(&environment, GraphEngine::Ladybug, true).unwrap();
        assert_eq!((again.action, again.changed), ("ready", false));
        fs::remove_dir_all(scratch).unwrap();
    }
}
