//! Shared delimiter splitting for checked sources and native rendering.
/// The frontmatter between the leading `---` line and the next, and the body
/// after it.
pub(crate) fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let (frontmatter, tail) = rest.split_at_checked(offset)?;
            return Some((frontmatter, tail.get(line.len()..)?));
        }
        offset += line.len();
    }
    None
}
