//! Detects absolute paths naming a user's home directory.

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

#[cfg(test)]
mod tests {
    use super::names_a_personal_directory;

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
}
