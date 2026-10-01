//! Hostile sources inside D2's limits: YAML aliases that expand past their
//! bytes, dependency graphs long or dense enough to exhaust the stack or the
//! output, and more diagnostics than a person can read.

use super::support::{MemoryTree, assert_refused, check_under};
use crate::limits::Limits;
use std::thread;

/// The valid skill's path.
const SKILL: &str = "skills/valid-skill/SKILL.md";

/// A reviewed skill `name` whose metadata requires `requires`.
fn skill(name: &str, requires: &[String]) -> String {
    let requires = if requires.is_empty() {
        String::new()
    } else {
        format!("  maestro.requires: {}\n", requires.join(";"))
    };
    format!(
        "---\nname: {name}\ndescription: Synthetic skill of a dependency graph.\nmetadata:\n  \
            maestro.schema: maestro-source/2\n  maestro.owner: \"@synthetic/knowledge\"\n  \
            maestro.maturity: reviewed\n  maestro.rows: chat.M019 descriptor\n  \
            maestro.workflows: ctm-question\n{requires}---\n\nSynthetic body.\n"
    )
}

/// The valid catalog plus the skills `names`, each requiring the skills
/// `requires` gives it.
fn with_skills(names: &[String], requires: impl Fn(usize) -> Vec<String>) -> MemoryTree {
    names
        .iter()
        .enumerate()
        .fold(MemoryTree::valid(), |tree, (index, name)| {
            tree.with(
                &format!("skills/{name}/SKILL.md"),
                &skill(name, &requires(index)),
            )
        })
}

/// The lines of `tree`'s refusal under the production limits.
fn refusal_lines(tree: &MemoryTree) -> Vec<String> {
    check_under(tree, &Limits::PRODUCTION)
        .unwrap_err()
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn yaml_aliases_expanding_past_twice_the_frontmatter_bytes_are_refused() {
    let anchor = vec!["a"; 100].join(", ");
    let aliases = vec!["*x"; 100].join(", ");
    let tree = MemoryTree::valid().edit(
        SKILL,
        "metadata:",
        &format!("list: &x [{anchor}]\nbomb: [{aliases}]\nmetadata:"),
    );
    let frontmatter = tree.text(SKILL).split("---\n").nth(1).unwrap().len();
    let expected = format!(
        "skills/valid-skill/SKILL.md: more than {} YAML nodes",
        2 * frontmatter
    );
    assert_refused(vec![("alias bomb", tree, &expected)]);
}

#[test]
fn a_chain_of_4095_resources_is_checked_on_a_1_mib_stack() {
    let names: Vec<String> = (0..4_091).map(|index| format!("chain-{index}")).collect();
    let last = names.len() - 1;
    let tree = with_skills(&names, |index| {
        if index == last {
            Vec::new()
        } else {
            vec![format!("skill:common/chain-{}", index + 1)]
        }
    });
    let checked = thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            check_under(
                &tree,
                &Limits {
                    archive_entries: 10_000,
                    ..Limits::PRODUCTION
                },
            )
            .map(|catalog| catalog.resources.len())
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(checked, Ok(4_095));
}

#[test]
fn production_walk_bound_refuses_the_large_graph_before_graph_checks() {
    let names: Vec<String> = (0..4_091).map(|index| format!("chain-{index}")).collect();
    let tree = with_skills(&names, |_| Vec::new());
    let error = check_under(&tree, &Limits::PRODUCTION).unwrap_err();
    assert!(
        error.to_string().contains("more than 4096 walk entries"),
        "{error}"
    );
}

#[test]
fn a_complete_graph_of_50_skills_is_one_cycle_diagnostic() {
    let names: Vec<String> = (0..50).map(|index| format!("node-{index:02}")).collect();
    let tree = with_skills(&names, |index| {
        names
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, name)| format!("skill:common/{name}"))
            .collect()
    });
    let cycles: Vec<String> = refusal_lines(&tree)
        .into_iter()
        .filter(|line| line.contains("dependency cycle"))
        .collect();
    assert_eq!(
        cycles,
        [
            "skills/node-00/SKILL.md: metadata.maestro.requires: dependency cycle among \
            skill:common/node-00, skill:common/node-01, skill:common/node-02, \
            skill:common/node-03, skill:common/node-04, \
            skill:common/node-05, skill:common/node-06, skill:common/node-07, \
            skill:common/node-08, skill:common/node-09 and 40 \
            more"
        ]
    );
}

#[test]
fn a_self_requirement_is_a_cycle() {
    let names = vec!["alone".to_owned()];
    let tree = with_skills(&names, |_| vec!["skill:common/alone".to_owned()]);
    assert_eq!(
        refusal_lines(&tree),
        [
            "skills/alone/SKILL.md: metadata.maestro.requires: dependency cycle among \
            skill:common/alone"
        ]
    );
}

#[test]
fn diagnostics_stop_at_1000_with_a_count_of_the_rest() {
    let tree = (0..1_500).fold(MemoryTree::valid(), |tree, index| {
        tree.with(&format!("core/agents/stray-{index:04}.txt"), "notes\n")
    });
    let lines = refusal_lines(&tree);
    assert_eq!(lines.len(), 1_001);
    assert_eq!(
        lines.first().map(String::as_str),
        Some(
            "core/agents/stray-0000.txt: not a registered v4 placement; nested/unknown areas \
            and unregistered trees refuse; migrate old or mixed layouts to maestro-source/2"
        )
    );
    assert_eq!(
        lines.last().map(String::as_str),
        Some("catalog: 500 more diagnostics not shown")
    );
}
