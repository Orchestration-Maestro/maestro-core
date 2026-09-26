//! The checks of a document's body, each with a page it flags and one it must
//! not: format-aware, so a short changelog, a small configuration file or an
//! index that says what each link holds is content, while a page whose body
//! is a link, a label, a menu or a prompt is not.

use super::support::{flag, flags, reason, rules};
use maestro_kernel::document::Outcome;

/// Twelve links, one a list item, and nothing else.
fn menu() -> String {
    let items = (1..=12)
        .map(|number| format!("- [Topic {number}](topic-{number}.html)\n"))
        .collect::<Vec<_>>()
        .concat();
    format!("# Topics\n\n{items}")
}

/// The reason `body.near-empty` gives a body of `words` words of prose.
fn near_empty(words: &str) -> String {
    format!(
        "its body holds {words} outside link labels, and no code block, list item or table \
         cell holds one: fewer than 8 is near-empty"
    )
}

#[test]
fn a_body_that_is_a_link_or_a_label_is_near_empty() {
    for (page, words) in [
        (
            "# Garden party\n\n[See every event](https://example.org/events)\n",
            "0 words",
        ),
        ("#### \n\nAttachments:\n", "1 word"),
        ("# Seeds\n\nThis folder holds the examples.\n", "5 words"),
        (
            "# A long heading that says a great deal about the nightly backups\n\n\
             [More](more.html)\n",
            "0 words",
        ),
        (
            "---\ncollection: notes\nrelease: 1.0\nsource_tree: web\n---\n\
             #### \n\nAttachments:\n",
            "1 word",
        ),
        (
            "# Diagram\n\n\
             ![The states a job moves through from its order to its end](states.png)\n",
            "0 words",
        ),
    ] {
        let found = flags(page);
        let flagged = flag(&found, "body.near-empty");
        assert_eq!(flagged.outcome, Outcome::NeedsReextraction, "{page}");
        assert_eq!(flagged.reason, near_empty(words), "{page}");
    }
}

#[test]
fn a_short_changelog_a_small_file_or_one_sentence_is_not_near_empty() {
    for page in [
        "# Changelog\n\n## 1.0.1\n\n- Fixed the timeout.\n\n## 1.0.0\n\n- First release.\n",
        "# requirements\n\n```text\nrequests\n```\n",
        "| Setting | Value |\n| --- | --- |\n| retries | 3 |\n",
        "# Backups\n\nThe backup runs every night at two in the morning.\n",
    ] {
        assert_eq!(rules(page), [""; 0], "{page}");
    }
}

#[test]
fn a_body_is_near_empty_below_eight_words_of_prose() {
    let seven = "# Note\n\nOne two three four five six seven.\n";
    assert_eq!(rules(seven), ["body.near-empty"]);
    let eight = "# Note\n\nOne two three four five six seven eight.\n";
    assert_eq!(rules(eight), [""; 0]);
}

#[test]
fn a_long_menu_of_links_is_navigation() {
    let found = flags(&menu());
    let navigation = flag(&found, "body.navigation-heavy");
    assert_eq!(navigation.outcome, Outcome::NeedsReextraction);
    assert_eq!(
        navigation.reason,
        "link labels are 100 % of its body's 24 words, over 12 links: from 90 % over 10 \
         links, a page is navigation"
    );
}

#[test]
fn an_index_that_says_what_each_link_holds_or_a_few_links_is_not_navigation() {
    let described = (1..=12)
        .map(|number| {
            format!(
                "- [Topic {number}](topic-{number}.html): when a job runs again after a delay\n"
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let few = "# Topics\n\n- [Topic 1](one.html)\n- [Topic 2](two.html)\n- [Topic 3](three.html)\n";
    for page in [format!("# Topics\n\n{described}"), few.to_owned()] {
        let found = rules(&page);
        assert!(
            !found.contains(&"body.navigation-heavy"),
            "{page}: {found:?}"
        );
    }
    assert_eq!(
        rules(few),
        ["body.near-empty"],
        "a few links alone are near-empty"
    );
}

#[test]
fn navigation_starts_at_ten_links_and_ninety_percent_of_the_words() {
    let links = |count: u32, label: &str| -> String {
        (1..=count)
            .map(|number| format!("- [{label}](topic-{number}.html)\n"))
            .collect::<Vec<_>>()
            .concat()
    };
    let navigation = |page: &str| rules(page).contains(&"body.navigation-heavy");
    assert!(navigation(&format!("# Topics\n\n{}", links(10, "Topic"))));
    assert!(!navigation(&format!("# Topics\n\n{}", links(9, "Topic"))));
    let label = "one two three four five six seven eight nine";
    let ten = "Each page below holds one of the topics we keep.";
    let eleven = "Each page below holds one of the many topics we keep.";
    assert!(navigation(&format!(
        "# Topics\n\n{ten}\n\n{}",
        links(10, label)
    )));
    assert!(!navigation(&format!(
        "# Topics\n\n{eleven}\n\n{}",
        links(10, label)
    )));
}

#[test]
fn an_application_error_where_content_should_be_is_flagged() {
    for page in [
        "# Status\n\nAn unexpected error has occurred.\n\nLoading\n",
        "# Downloads\n\nSomething went wrong. Please try again later.\n\n\
         The downloads of this product are listed here once the list has loaded.\n",
        "# 404 Not Found\n\nThe page you asked for is not on this server any more.\n",
    ] {
        let found = flags(page);
        let error = flag(&found, "page.application-error");
        assert_eq!(error.outcome, Outcome::NeedsReextraction, "{page}");
        assert_eq!(
            error.reason,
            "an application's error message stands where content should be, in 1 of its \
             paragraphs and headings",
            "{page}"
        );
    }
    let twice = "# Something went wrong\n\nAn error occurred.\n\nThe list loads here.\n";
    assert_eq!(
        reason(twice, "page.application-error"),
        "an application's error message stands where content should be, in 2 of its \
         paragraphs and headings"
    );
}

#[test]
fn an_error_a_page_explains_is_not_an_application_error() {
    for page in [
        "# Deploy fails\n\n\
         The deploy fails with an Internal Server Error when the token expires.\n\n\
         ```text\nHTTP/1.1 500 Internal Server Error\n```\n\n\
         > Internal Server Error\n\n\
         Renew the token, then deploy again.\n",
        "# 502 Bad Gateway\n\n\
         A proxy answers 502 Bad Gateway when the server behind it sends no valid answer in \
         time. Check first that the application server runs and listens on the port the proxy \
         forwards to. Then compare the proxy's timeout with the time the slowest request \
         takes, and raise it if requests end early. Restart the proxy once its configuration \
         changes.\n",
    ] {
        assert_eq!(rules(page), [""; 0], "{page}");
    }
}

#[test]
fn an_error_heading_counts_over_a_body_of_fewer_than_fifty_words() {
    let page = |words: usize| format!("# 502 Bad Gateway\n\n{}\n", vec!["word"; words].join(" "));
    assert!(rules(&page(49)).contains(&"page.application-error"));
    assert!(!rules(&page(50)).contains(&"page.application-error"));
    // A paragraph that is an error message counts over any body.
    let paragraph = format!(
        "# Status\n\nInternal server error\n\n{}\n",
        vec!["word"; 60].join(" ")
    );
    assert!(rules(&paragraph).contains(&"page.application-error"));
}

#[test]
fn a_sign_in_or_challenge_page_is_flagged() {
    for (page, which, words) in [
        (
            "# Sign in\n\nUsername\n\nPassword\n\n\
             [Forgot your password?](https://example.org/reset)\n",
            "heading",
            "5 words",
        ),
        (
            "Just a moment...\n\nChecking your browser before accessing example.org.\n",
            "paragraph",
            "9 words",
        ),
        (
            "## Please enable JavaScript and cookies to continue\n",
            "heading",
            "0 words",
        ),
    ] {
        let found = flags(page);
        let prompt = flag(&found, "page.sign-in");
        assert_eq!(prompt.outcome, Outcome::NeedsReextraction, "{page}");
        assert_eq!(
            prompt.reason,
            format!(
                "its first {which} is a sign-in or challenge prompt, and its body holds \
                 {words}: fewer than 50 is the prompt's page, not the document"
            ),
            "{page}"
        );
    }
}

#[test]
fn a_sign_in_page_holds_fewer_than_fifty_words() {
    let page = |words: usize| format!("# Sign in\n\n{}\n", vec!["word"; words].join(" "));
    assert!(rules(&page(49)).contains(&"page.sign-in"));
    assert!(!rules(&page(50)).contains(&"page.sign-in"));
}

#[test]
fn a_prompt_a_page_quotes_is_not_a_sign_in_page() {
    let page = "> Sign in\n\nThe sign-in page asks for a user name and a password.\n";
    assert_eq!(rules(page), [""; 0]);
}

#[test]
fn a_guide_about_signing_in_is_not_a_sign_in_page() {
    let page = "# Log in\n\n\
        Open the web client in a browser and type the name of the server you were given. \
        Enter your user name and password as your administrator created them, then choose \
        the environment you work in. The client opens the workspaces you had open when \
        you last signed out, and it asks before it restores any of them, so that you can \
        start again from an empty workspace when you prefer.\n";
    assert_eq!(rules(page), [""; 0]);
}
