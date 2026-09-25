//! Helpers for the repository's policy tests: the files the repository holds,
//! counted lines, personal directories and Markdown links. The policies live
//! in `tests/policies.rs`, so CI enforces them.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

/// Directories that hold history, downloaded tools, build output or mutation
/// test output, never repository content.
const SKIPPED: [&str; 5] = [".git", ".tools", "target", "mutants.out", "mutants.out.old"];

/// The repository root, two levels above this crate.
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every file under `root` outside `.git`, `.tools`, `target` and the mutation
/// test output, relative and sorted. A walk rather than `git ls-files`:
/// mutation testing runs in a copy without `.git`.
///
/// # Errors
/// Any unreadable directory: a policy that cannot see a file must fail.
pub fn repository_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() && !SKIPPED.iter().any(|name| entry.file_name() == *name) {
                pending.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                files.push(
                    path.strip_prefix(root)
                        .map_err(io::Error::other)?
                        .to_path_buf(),
                );
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Lines that are neither blank nor `//` comments: the measure of a file's size.
#[must_use]
pub fn counted_lines(text: &str) -> usize {
    text.lines()
        .map(str::trim_start)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .count()
}

/// Whether the text names a person's home directory: a name, then a
/// separator, under `/home/`, `/Users/` or a Windows drive's `Users` folder.
#[must_use]
pub fn names_a_personal_directory(text: &str) -> bool {
    let homes = [
        (["/", "home", "/"].concat(), '/'),
        (["/", "Users", "/"].concat(), '/'),
        (["C:", "\\", "Users", "\\"].concat(), '\\'),
    ];
    homes.iter().any(|(prefix, separator)| {
        text.match_indices(prefix.as_str()).any(|(at, _)| {
            let rest = &text[at + prefix.len()..];
            let name = rest
                .find(|character: char| {
                    !(character.is_alphanumeric() || matches!(character, '.' | '_' | '-'))
                })
                .unwrap_or(rest.len());
            name > 0 && rest[name..].starts_with(*separator)
        })
    })
}

/// Every relative link or image in `file` whose target file or heading anchor
/// does not exist, as `file: target` lines.
///
/// # Errors
/// An unreadable Markdown file.
pub fn broken_links(root: &Path, file: &Path) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(root.join(file))?;
    let mut problems = Vec::new();
    for target in link_targets(&text) {
        if target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        let (path, anchor) = target.split_once('#').unwrap_or((target.as_str(), ""));
        let linked = if path.is_empty() {
            root.join(file)
        } else {
            root.join(file.parent().unwrap_or(Path::new(""))).join(path)
        };
        let markdown = linked
            .extension()
            .is_some_and(|extension| extension == "md");
        let found = if anchor.is_empty() || !markdown {
            linked.exists()
        } else {
            fs::read_to_string(&linked).is_ok_and(|text| anchors(&text).contains(anchor))
        };
        if !found {
            problems.push(format!("{}: {target}", file.display()));
        }
    }
    Ok(problems)
}

/// The destinations of the links and images in a Markdown text.
fn link_targets(text: &str) -> Vec<String> {
    Parser::new(text)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                Some(dest_url.to_string())
            }
            _ => None,
        })
        .collect()
}

/// The anchors GitHub gives a Markdown text's headings, duplicates numbered.
fn anchors(text: &str) -> BTreeSet<String> {
    let mut anchors = BTreeSet::new();
    let mut heading: Option<String> = None;
    for event in Parser::new(text) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::Text(part) | Event::Code(part) => {
                if let Some(title) = heading.as_mut() {
                    title.push_str(&part);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(title) = heading.take() {
                    let anchor = unused_anchor(&anchors, &slug(&title));
                    anchors.insert(anchor);
                }
            }
            _ => {}
        }
    }
    anchors
}

/// The first of `base`, `base-1`, `base-2`... that `anchors` does not hold yet.
fn unused_anchor(anchors: &BTreeSet<String>, base: &str) -> String {
    let (mut anchor, mut number) = (base.to_owned(), 0);
    while anchors.contains(&anchor) {
        number += 1;
        anchor = format!("{base}-{number}");
    }
    anchor
}

/// GitHub's heading slug: lower case; letters, digits, `_` and `-` kept;
/// spaces as `-`; everything else dropped.
fn slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter_map(|character| match character {
            ' ' => Some('-'),
            kept if kept.is_alphanumeric() || kept == '_' || kept == '-' => Some(kept),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, process};

    /// A fresh directory under the system's temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("policy-{name}-{}", process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn counted_lines_skip_blank_lines_and_line_comments() {
        assert_eq!(
            counted_lines("fn a() {\n\n  // note\n/// doc\n  let b = 1;\n}\n"),
            3
        );
    }

    #[test]
    fn personal_directories_need_a_name_and_a_slash() {
        let home = ["/", "home", "/"].concat();
        let users = ["/", "Users", "/"].concat();
        assert!(names_a_personal_directory(&format!("x {home}alice/notes")));
        assert!(names_a_personal_directory(&format!("{users}bob/")));
        let windows = ["C:", "\\", "Users", "\\"].concat();
        assert!(names_a_personal_directory(&format!(
            "{windows}carol\\notes"
        )));
        assert!(!names_a_personal_directory(&format!(
            "{home}<name>/ ~/workspace /usr/bin"
        )));
        assert!(!names_a_personal_directory(&home));
        assert!(!names_a_personal_directory(&windows));
        assert!(!names_a_personal_directory(&format!("{home}/twice")));
        assert!(!names_a_personal_directory(&format!("{home}alice")));
    }

    #[test]
    fn headings_get_github_anchors_with_numbered_duplicates() {
        let found =
            anchors("# 1.3 Check, compile\n## `knowledge_search` tool\n## Notes\n## Notes\n");
        let expected = [
            "13-check-compile",
            "knowledge_search-tool",
            "notes",
            "notes-1",
        ];
        assert_eq!(
            found,
            expected
                .into_iter()
                .map(String::from)
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn link_targets_include_images_and_skip_code() {
        let text = "[a](b.md#c) ![i](img.svg)\n\n```\n[x](y.md)\n```\n\n`[z](w.md)`\n";
        assert_eq!(link_targets(text), ["b.md#c", "img.svg"]);
    }

    #[test]
    fn broken_links_name_missing_files_and_anchors() {
        let root = scratch("links");
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/b.md"), "# Present\n").unwrap();
        let links = "[1](b.md#present) [2](b.md#absent) [3](c.md) [4](#here) [5](#gone) \
                     [6](https://example.org/x) [7](b.md)";
        fs::write(root.join("docs/a.md"), format!("# Here\n\n{links}\n")).unwrap();
        let problems = broken_links(&root, Path::new("docs/a.md")).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            problems,
            [
                "docs/a.md: b.md#absent",
                "docs/a.md: c.md",
                "docs/a.md: #gone"
            ]
        );
    }

    #[test]
    fn repository_files_skip_git_tools_and_build_output() {
        let root = scratch("files");
        for dir in [
            ".git",
            ".tools/bin",
            "target/debug",
            "crates/a/target",
            "crates/a/src",
            "mutants.out/log",
            "mutants.out.old",
        ] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in [
            ".git/HEAD",
            ".tools/bin/x",
            "target/debug/y",
            "crates/a/target/z",
            "crates/a/src/lib.rs",
            "mutants.out/log/m.log",
            "mutants.out.old/debug.log",
            "README.md",
        ] {
            fs::write(root.join(file), "").unwrap();
        }
        let files = repository_files(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            files,
            [
                PathBuf::from("README.md"),
                PathBuf::from("crates/a/src/lib.rs")
            ]
        );
    }
}
