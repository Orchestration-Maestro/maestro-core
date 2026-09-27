//! Counts significant source lines for policy size checks.

/// Lines that are neither blank nor `//` comments: the measure of a file's size.
#[must_use]
pub fn counted_lines(text: &str) -> usize {
    text.lines()
        .map(str::trim_start)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .count()
}

#[cfg(test)]
mod tests {
    use super::counted_lines;

    #[test]
    fn counted_lines_skip_blank_lines_and_line_comments() {
        assert_eq!(
            counted_lines("fn a() {\n\n  // note\n/// doc\n  let b = 1;\n}\n"),
            3
        );
    }
}
