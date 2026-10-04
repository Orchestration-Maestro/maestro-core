//! The shared repair text is an executable command, not an invented namespace.
use crate::cli::args::Arguments;
use clap::Parser as _;
use maestro_kernel::facts::PROJECTION_REBUILD_REPAIR;

#[test]
fn graph_rebuild_shared_repair_command_parses_literally() {
    let command = PROJECTION_REBUILD_REPAIR.rsplit_once("run ").unwrap().1;
    assert_eq!(command, "maestro knowledge graph rebuild");
    let parsed = Arguments::try_parse_from(command.split_whitespace());
    assert!(parsed.is_ok(), "{parsed:?}");
}
