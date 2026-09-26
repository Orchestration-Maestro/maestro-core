//! `text.suspected-secret`: a credential in a format whose shape alone
//! identifies it quarantines its document, and the reason names the format
//! and the line, never the text. Each format has examples it flags and ones
//! it must not, documentation's placeholders among them. The credentials are
//! put together when the tests run, so that no secret scanner finds one in
//! this file.

use super::support::{flag, flags, reason, rules};
use maestro_kernel::document::Outcome;

/// The characters of a token's random part.
const BASE62: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// The characters of the random part of an AWS access key ID.
const BASE32: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// The decimal digits.
const DIGITS: &str = "0123456789";

/// The characters of base64.
const BASE64: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `length` characters of `alphabet`, each `stride` places after the one
/// before: random-looking, with no character twice in a row and none that
/// follows the one before in ASCII, as a placeholder's would.
fn drawn(alphabet: &str, length: usize, stride: usize) -> String {
    let characters: Vec<char> = alphabet.chars().collect();
    (0..length)
        .map(|index| characters[(index * stride + 1) % characters.len()])
        .collect()
}

/// A PEM block of `label` around `body`.
fn armor(label: &str, body: &str) -> String {
    let dashes = "-".repeat(5);
    format!("{dashes}BEGIN {label}{dashes}\n{body}\n{dashes}END {label}{dashes}")
}

/// `length` base64 characters, 64 a line: a key block's body.
fn key_body(length: usize) -> String {
    drawn(BASE64, length, 5)
        .as_bytes()
        .chunks(64)
        .map(|line| String::from_utf8(line.to_vec()).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

/// An AWS access key ID.
fn aws_key() -> String {
    ["AK", "IA", &drawn(BASE32, 16, 5)].concat()
}

/// A GitHub token in the classic format.
fn github_token() -> String {
    ["gh", "p_", &drawn(BASE62, 36, 7)].concat()
}

/// A GitLab personal access token whose random part has `length` characters.
fn gitlab_token(length: usize) -> String {
    ["gl", "pat-", &drawn(BASE62, length, 7)].concat()
}

/// A Slack bot token: two numbers of `digits` digits each, and a secret of
/// `secret` letters and digits.
fn slack_token(digits: usize, secret: usize) -> String {
    slack_parts(digits, digits, &drawn(BASE62, secret, 7))
}

/// A Slack bot token of two numbers, of `first` and `second` digits, and
/// `secret`.
fn slack_parts(first: usize, second: usize, secret: &str) -> String {
    [
        "xo",
        "xb-",
        &drawn(DIGITS, first, 3),
        "-",
        &drawn(DIGITS, second, 7),
        "-",
        secret,
    ]
    .concat()
}

/// A page whose line 4 is `text`, in a code block.
fn in_code(text: &str) -> String {
    format!("# Setup\n\n```text\n{text}\n```\n")
}

/// The reason of one suspected secret, in `format`, on `line`.
fn one(format: &str, line: usize) -> String {
    format!(
        "1 suspected secret, quarantined until someone reviews the document: {format} \
         (line {line})"
    )
}

/// Asserts that the page `page` is quarantined as holding one suspected
/// secret, in `format`, on line 4.
fn flagged_on_line_four(page: &str, format: &str) {
    let found = flags(page);
    let secret = flag(&found, "text.suspected-secret");
    assert_eq!(secret.outcome, Outcome::Quarantined, "{page}");
    assert_eq!(secret.reason, one(format, 4), "{page}");
}

#[test]
fn a_private_key_block_is_a_suspected_secret() {
    let headers = format!(
        "Proc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,{}\n\n{}",
        drawn(BASE62, 32, 7),
        key_body(128)
    );
    let escaped = armor("PRIVATE KEY", &key_body(64)).replace('\n', "\\n");
    for page in [
        in_code(&armor("RSA PRIVATE KEY", &key_body(192))),
        in_code(&armor("OPENSSH PRIVATE KEY", &key_body(64))),
        in_code(&armor("RSA PRIVATE KEY", &headers)),
        in_code(&armor(
            "PGP PRIVATE KEY BLOCK",
            &format!("Version: 1\n\n{}", key_body(128)),
        )),
        in_code(&format!("{{\"private_key\": \"{escaped}\"}}")),
    ] {
        flagged_on_line_four(&page, "PEM private key block");
    }
}

#[test]
fn a_key_block_needs_sixty_four_base64_characters() {
    let block = |length: usize| in_code(&armor("PRIVATE KEY", &key_body(length)));
    assert_eq!(rules(&block(63)), [""; 0]);
    assert_eq!(rules(&block(64)), ["text.suspected-secret"]);
}

#[test]
fn a_placeholder_a_public_key_or_a_certificate_is_not_a_private_key() {
    let dashes = "-".repeat(5);
    let unterminated = format!("{dashes}BEGIN RSA PRIVATE KEY{dashes}\n{}", key_body(192));
    for page in [
        in_code(&armor("RSA PRIVATE KEY", "...")),
        in_code(&armor("PRIVATE KEY", "<your private key>")),
        in_code(&armor("PRIVATE KEY", &format!("{}...", key_body(128)))),
        in_code(&armor("CERTIFICATE", &key_body(192))),
        in_code(&armor("PUBLIC KEY", &key_body(192))),
        in_code(&armor("Private Key", &key_body(192))),
        in_code(&armor("rsa PRIVATE KEY", &key_body(192))),
        in_code(&unterminated),
    ] {
        assert_eq!(rules(&page), [""; 0], "{page}");
    }
}

#[test]
fn an_aws_access_key_id_is_a_suspected_secret() {
    let temporary = aws_key().replacen("AK", "AS", 1);
    for key in [aws_key(), temporary] {
        flagged_on_line_four(
            &in_code(&format!("aws_access_key_id = {key}")),
            "AWS access key ID",
        );
    }
}

#[test]
fn the_documented_example_key_or_a_longer_word_is_not_an_aws_access_key_id() {
    let example = ["AK", "IA", "IOSFODNN7", "EXAMPLE"].concat();
    let lowercase = ["AK", "IA", &drawn(BASE32, 16, 5).to_lowercase()].concat();
    let digit_one = aws_key().replacen("AKIAB", "AKIA1", 1);
    for text in [
        format!("aws_access_key_id = {example}"),
        format!("{}Z", aws_key()),
        format!("X{}", aws_key()),
        lowercase,
        digit_one,
    ] {
        assert_eq!(rules(&in_code(&text)), [""; 0], "{text}");
    }
}

#[test]
fn a_github_token_is_a_suspected_secret() {
    let oauth = github_token().replacen("p_", "o_", 1);
    for token in [github_token(), oauth, fine_grained(22, 59, false)] {
        flagged_on_line_four(
            &in_code(&format!("export GITHUB_TOKEN={token}")),
            "GitHub token",
        );
    }
}

/// A fine-grained GitHub token whose parts have `first` and `second`
/// characters, the second `repeated` or drawn.
fn fine_grained(first: usize, second: usize, repeated: bool) -> String {
    let second = if repeated {
        "x".repeat(second)
    } else {
        drawn(BASE62, second, 11)
    };
    ["github", "_pat_", &drawn(BASE62, first, 7), "_", &second].concat()
}

#[test]
fn a_placeholder_or_a_token_inside_a_word_is_not_a_github_token() {
    for text in [
        ["gh", "p_", &"x".repeat(36)].concat(),
        ["gh", "p_<your token>"].concat(),
        format!("x{}", github_token()),
        format!("{}x", github_token()),
        github_token().replacen("p_", "q_", 1),
        github_token().replacen('F', "-", 1),
        fine_grained(21, 60, false),
        fine_grained(22, 58, false),
        fine_grained(22, 59, true),
    ] {
        assert_eq!(rules(&in_code(&text)), [""; 0], "{text}");
    }
}

#[test]
fn a_gitlab_token_is_a_suspected_secret_from_twenty_characters() {
    let deploy = gitlab_token(20).replacen("glpat-", "gldt-", 1);
    for token in [gitlab_token(20), gitlab_token(32), deploy] {
        flagged_on_line_four(&in_code(&format!("token: {token}")), "GitLab token");
    }
    assert_eq!(rules(&in_code(&gitlab_token(19))), [""; 0]);
}

#[test]
fn an_ascending_placeholder_is_not_a_gitlab_token() {
    let ascending = ["gl", "pat-", "abcdefghij0123456789"].concat();
    assert_eq!(rules(&in_code(&ascending)), [""; 0]);
}

#[test]
fn a_slack_token_is_a_suspected_secret() {
    for token in [slack_token(10, 16), slack_token(12, 24)] {
        flagged_on_line_four(&in_code(&format!("SLACK_TOKEN={token}")), "Slack token");
    }
}

#[test]
fn a_placeholder_or_a_short_part_is_not_a_slack_token() {
    for text in [
        ["xo", "xb-your-bot-token"].concat(),
        slack_token(9, 24),
        slack_parts(10, 9, &drawn(BASE62, 24, 7)),
        slack_token(10, 15),
        slack_token(10, 24).replacen('-', "-a", 1),
        slack_parts(10, 10, &"x".repeat(24)),
        slack_token(10, 24).replacen("xoxb", "xoxz", 1),
    ] {
        assert_eq!(rules(&in_code(&text)), [""; 0], "{text}");
    }
}

#[test]
fn documentation_placeholders_are_not_secrets() {
    for text in [
        "password: <your password>",
        "token: <your token>",
        "aws_secret_access_key: <your secret access key>",
        "Authorization: Bearer ${TOKEN}",
    ] {
        assert_eq!(rules(&in_code(text)), [""; 0], "{text}");
    }
}

#[test]
fn the_reason_names_each_format_and_its_lines_and_never_the_text() {
    let (aws, github, gitlab) = (aws_key(), github_token(), gitlab_token(24));
    let page = format!(
        "---\ntoken: {gitlab}\n---\n# Keys\n\n```text\n{aws}\n{github} {aws}\n```\n\n\
         {}\n",
        armor("EC PRIVATE KEY", &key_body(96))
    );
    let found = reason(&page, "text.suspected-secret");
    assert_eq!(
        found,
        "5 suspected secrets, quarantined until someone reviews the document: PEM private key \
         block (line 11); AWS access key ID (lines 7, 8); GitHub token (line 8); GitLab token \
         (line 2)"
    );
    for secret in [aws, github, gitlab] {
        assert!(!found.contains(&secret));
    }
}
