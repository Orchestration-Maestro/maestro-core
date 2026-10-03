//! The held-handle boundary scanner and its regression cases.
use std::path::Path;

/// Refuse unsafe tokens and unsafe-code lint attributes, not comments or literals.
pub(super) fn unsafe_boundary_violation(text: &str) -> bool {
    let tokens = tokens(text);
    if tokens.contains(&"unsafe") {
        return true;
    }
    for (index, token) in tokens.iter().enumerate() {
        if *token != "#" {
            continue;
        }
        let start = index + 1 + usize::from(tokens.get(index + 1) == Some(&"!"));
        if tokens.get(start) == Some(&"[") {
            let end = group_end(&tokens, start, "[", "]");
            if lint_violation(&tokens[start + 1..end]) {
                return true;
            }
        }
    }
    false
}

/// Only a real cfg(test) module door admits the native PTY test-support subtree.
pub(super) fn cfg_test_module(text: &str, module: &str) -> bool {
    let tokens = tokens(text);
    tokens
        .windows(3)
        .filter(|window| *window == ["mod", module, ";"])
        .count()
        == 1
        && tokens
            .windows(10)
            .any(|window| window == ["#", "[", "cfg", "(", "test", ")", "]", "mod", module, ";"])
}

#[test]
fn unsafe_boundary_pty_support_requires_a_real_test_only_door() {
    assert!(cfg_test_module("#[cfg(test)] mod tests;", "tests"));
    for source in [
        "mod tests;",
        "// #[cfg(test)] mod tests;",
        "#[cfg(unix)] mod tests;",
        "#[cfg(test)] mod tests; #[cfg(not(test))] mod tests;",
        "#[cfg(test)] mod tests; mod tests;",
    ] {
        assert!(!cfg_test_module(source, "tests"));
    }
}

/// Check lint lists, including those nested in `cfg_attr`.
fn lint_violation(tokens: &[&str]) -> bool {
    tokens.windows(2).enumerate().any(|(index, pair)| {
        if !["allow", "expect", "warn"].contains(&pair[0]) || pair[1] != "(" {
            return false;
        }
        let end = group_end(tokens, index + 1, "(", ")");
        tokens[index + 2..end].contains(&"unsafe_code")
    })
}

/// Index of the matching delimiter, or the end for incomplete source.
fn group_end(tokens: &[&str], start: usize, open: &str, close: &str) -> usize {
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if *token == open {
            depth += 1;
        } else if *token == close {
            depth -= 1;
            if depth == 0 {
                return index;
            }
        }
    }
    tokens.len()
}

/// Lex identifiers and punctuation while skipping Rust comments and literals.
fn tokens(mut text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    while !text.is_empty() {
        text = text.trim_start();
        if text.is_empty() {
            break;
        }
        if text.starts_with("//") {
            text = &text[text.find('\n').unwrap_or(text.len())..];
        } else if text.starts_with("/*") {
            text = &text[comment_end(text)..];
        } else if let Some(length) = literal_end(text) {
            text = &text[length..];
        } else {
            let first = text.chars().next().unwrap();
            let length = if first.is_alphanumeric() || first == '_' {
                text.find(|character: char| !character.is_alphanumeric() && character != '_')
                    .unwrap_or(text.len())
            } else {
                first.len_utf8()
            };
            result.push(&text[..length]);
            text = &text[length..];
        }
    }
    result
}

/// Nested block comments end only when the outermost comment closes.
fn comment_end(text: &str) -> usize {
    let bytes = text.as_bytes();
    let (mut index, mut depth) = (2, 1);
    while index + 1 < bytes.len() {
        match &bytes[index..index + 2] {
            b"/*" => {
                depth += 1;
                index += 2;
            }
            b"*/" => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    return index;
                }
            }
            _ => index += 1,
        }
    }
    text.len()
}

/// Raw strings accept arbitrary hash counts; ordinary strings honor escapes.
fn literal_end(text: &str) -> Option<usize> {
    let raw = text
        .strip_prefix('b')
        .or_else(|| text.strip_prefix('c'))
        .unwrap_or(text);
    if let Some(rest) = raw.strip_prefix('r') {
        let hashes = rest.bytes().take_while(|byte| *byte == b'#').count();
        if rest.as_bytes().get(hashes) == Some(&b'"') {
            let start = text.len() - rest.len() + hashes + 1;
            let closing = format!("\"{}", "#".repeat(hashes));
            return Some(
                text[start..]
                    .find(&closing)
                    .map_or(text.len(), |index| start + index + closing.len()),
            );
        }
    }
    if text.starts_with('"') {
        return Some(quoted_end(text));
    }
    // A lifetime is not a character literal: only skip a closed single character.
    if let Some(rest) = text.strip_prefix('\'') {
        let first = rest.chars().next()?;
        if first == '\\' {
            return rest[1..].find('\'').map(|index| index + 3);
        }
        if rest[first.len_utf8()..].starts_with('\'') {
            return Some(first.len_utf8() + 2);
        }
    }
    None
}

/// Find an unescaped closing quote in a cooked string.
fn quoted_end(text: &str) -> usize {
    let mut escaped = false;
    for (index, character) in text.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return index + 1;
        }
    }
    text.len()
}

#[test]
fn unsafe_boundary_policy_rejects_formatted_blocks_and_lint_escape() {
    for planted in [
        ["unsafe", " { }"].concat(),
        ["unsafe", "\n{ }"].concat(),
        ["#[allow(", "unsafe", "_code)]"].concat(),
    ] {
        assert!(unsafe_boundary_violation(&planted));
    }
    assert!(!unsafe_boundary_violation("fn safe() {}"));
}

#[test]
fn unsafe_boundary_policy_rejects_review_probe_and_every_unsafe_token() {
    let probe =
        "#[expect(unsafe_code, reason = \"review probe\")]\npub unsafe fn review_probe() {}";
    assert!(unsafe_boundary_violation(probe), "review probe escaped");
    for planted in [
        "unsafe fn f() {}",
        "unsafe impl Send for X {}",
        "unsafe trait T {}",
        "unsafe extern \"C\" {}",
        "#[unsafe(no_mangle)] fn f() {}",
        "#![warn(unsafe_code)]",
        "#[expect(unsafe_code)] fn f() {}",
        "#[cfg_attr(test, allow(unsafe_code))] fn f() {}",
    ] {
        assert!(unsafe_boundary_violation(planted), "escaped: {planted}");
    }
}

#[test]
fn unsafe_boundary_policy_preserves_strengthening_lint_attributes() {
    for strengthening in [
        "#![forbid(unsafe_code)]",
        "#[deny(unsafe_code)] fn safe() {}",
    ] {
        assert!(!unsafe_boundary_violation(strengthening));
    }
}

#[test]
fn unsafe_boundary_policy_skips_comments_literals_and_identifier_substrings() {
    for harmless in [
        "// unsafe { }\nfn safe() {}",
        "/* unsafe fn f() {} /* #[allow(unsafe_code)] */ */ fn safe() {}",
        r#"let text = "unsafe { } #[allow(unsafe_code)]";"#,
        r####"let text = br###"unsafe fn f() {} \" #[expect(unsafe_code)]"###;"####,
        "let unsafe_name = 0; let unsafe_code = 1;",
        "fn f<'a>(x: &'a str) {}",
        "let quote = '\"'; let escaped = '\\'';",
    ] {
        assert!(
            !unsafe_boundary_violation(harmless),
            "false refusal: {harmless}"
        );
    }
    assert!(unsafe_boundary_violation("fn f<'a>() { unsafe {} }"));
    assert!(unsafe_boundary_violation(
        r##"let s = r#"safe"#; unsafe {}"##
    ));
}

/// Permit only the production boundary and the explicitly gated privilege fixture.
pub(super) fn unsafe_file_violation(file: &Path, text: &str) -> bool {
    let permitted = [
        "crates/maestro-filesystem/src/windows_security.rs",
        "crates/maestro-filesystem/src/windows_test_security.rs",
        // C05k: test-only pre_exec and ConPTY receipts; production stays forbidden.
        "crates/maestro/src/cli/init/tests/pty/unix.rs",
        "crates/maestro/src/cli/init/tests/pty/windows.rs",
    ];
    !permitted.iter().any(|path| file == Path::new(path)) && unsafe_boundary_violation(text)
}

/// The privilege fixture has exactly one declaration, guarded by both test and Windows.
pub(super) fn privilege_support_is_gated(text: &str) -> bool {
    let tokens = tokens(text);
    let declarations = tokens
        .windows(3)
        .filter(|window| *window == ["mod", "windows_test_security", ";"])
        .count();
    declarations == 1
        && tokens.windows(15).any(|window| {
            window
                == [
                    "#",
                    "[",
                    "cfg",
                    "(",
                    "all",
                    "(",
                    "test",
                    ",",
                    "windows",
                    ")",
                    ")",
                    "]",
                    "mod",
                    "windows_test_security",
                    ";",
                ]
        })
}

#[test]
fn unsafe_test_support_remains_exactly_gated_and_other_test_files_refuse() {
    let gated = "#[cfg(all(test, windows))] mod windows_test_security;";
    assert!(privilege_support_is_gated(gated));
    for invalid in [
        "mod windows_test_security;".to_owned(),
        "#[cfg(test)] mod windows_test_security;".to_owned(),
        "#[cfg(windows)] mod windows_test_security;".to_owned(),
        format!("{gated}\nmod windows_test_security;"),
    ] {
        assert!(!privilege_support_is_gated(&invalid));
    }
    for path in [
        "crates/maestro-filesystem/src/other_tests.rs",
        "crates/other/src/windows_test_security.rs",
    ] {
        assert!(unsafe_file_violation(
            Path::new(path),
            "#[cfg(test)] fn probe() { unsafe {} }"
        ));
    }
}
