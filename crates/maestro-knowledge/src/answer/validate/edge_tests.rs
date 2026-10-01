use super::{bundle, rejected_literals, trim_token_edges, unescaped, unsupported_literals};

#[test]
fn a_line_that_only_starts_with_three_backticks_is_still_checked() {
    let evidence = bundle("The scheduler stops when asked.");
    assert_eq!(
        rejected_literals("```halt``` stops the scheduler [1].", &evidence),
        Some(vec!["halt".to_owned()])
    );
    assert_eq!(
        unsupported_literals("Stop it:\n```bash\nhalt\n```", "How?", &evidence),
        ["halt"]
    );
}

#[test]
fn french_guillemets_around_a_literal_are_edge_punctuation() {
    let evidence = bundle("Le service lit sa configuration dans «/opt/ctm».");
    assert_eq!(
        rejected_literals(
            "Le service lit sa configuration dans «/opt/ctm» [1].",
            &evidence
        ),
        None
    );
    assert_eq!(
        rejected_literals(
            "Le service lit sa configuration dans «/opt/autre» [1].",
            &evidence
        ),
        Some(vec!["/opt/autre".to_owned()])
    );
}

#[test]
fn a_copied_prompt_escape_of_angle_brackets_is_the_bracket() {
    let evidence = bundle("Pass the <job> name to the tool.");
    assert_eq!(
        rejected_literals(
            r"Pass the `\u003cjob\u003e` name to the tool [1].",
            &evidence
        ),
        None
    );
    assert_eq!(unescaped(r"\u003ca\u003e \n"), r"<a> \n");
}

#[test]
fn a_named_entity_makes_a_token_a_literal() {
    let reply = "Stop it with tool&nbsp;-FORCE now [1].";
    assert_eq!(
        rejected_literals(reply, &bundle("Stop it with the tool now.")),
        Some(vec!["tool&nbsp;-FORCE".to_owned()])
    );
    assert_eq!(
        rejected_literals(reply, &bundle("Stop it with tool&nbsp;-FORCE now.")),
        None
    );
    assert_eq!(
        rejected_literals(
            "Wait a&ndash;b or a&;b then stop [1].",
            &bundle("Wait then stop.")
        ),
        Some(vec!["a&ndash;b".to_owned()])
    );
}

#[test]
fn a_trailing_trim_keeps_a_relative_paths_leading_dots() {
    assert_eq!(trim_token_edges("\"../up\"."), "../up");
    assert_eq!(trim_token_edges(".../x"), "../x");
    assert_eq!(trim_token_edges("x./"), "x./");
}
