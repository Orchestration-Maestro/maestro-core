//! Checks relative Markdown links and heading anchors.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::{collections::BTreeSet, fs, io, path::Path};

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
    use crate::conventions::test_support::scratch;

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
    fn broken_links_name_missing_files_and_anchors() -> io::Result<()> {
        let root = scratch("links")?;
        fs::create_dir_all(root.join("docs"))?;
        fs::write(root.join("docs/b.md"), "# Present\n")?;
        let links = "[1](b.md#present) [2](b.md#absent) [3](c.md) [4](#here) [5](#gone) \
                     [6](https://example.org/x) [7](b.md)";
        fs::write(root.join("docs/a.md"), format!("# Here\n\n{links}\n"))?;
        let problems = broken_links(&root, Path::new("docs/a.md"))?;
        fs::remove_dir_all(&root)?;
        assert_eq!(
            problems,
            [
                "docs/a.md: b.md#absent",
                "docs/a.md: c.md",
                "docs/a.md: #gone"
            ]
        );
        Ok(())
    }
}
