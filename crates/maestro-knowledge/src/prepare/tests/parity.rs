//! The parity fixtures: how their file writes them, and what the built-in
//! set holds.

use super::super::parity::{Fixture, fixtures, parse};

#[test]
fn parts_and_runs_repeat_as_many_times_as_they_say() {
    let text = r##"{
      "profile": "sha256:0",
      "fixtures": [
        {"name": "short", "canary": true, "input": ["ab"], "ids": [0, 7, 2]},
        {"name": "long", "input": ["# T\n\n", {"repeat": "a ", "times": 3}, "z"],
         "ids": [0, 5, {"repeat": 10, "times": 3}, 9, 2]}
      ]
    }"##;
    assert_eq!(
        parse(text).unwrap(),
        [
            Fixture {
                name: "short".to_owned(),
                input: "ab".to_owned(),
                ids: vec![0, 7, 2],
                canary: true,
            },
            Fixture {
                name: "long".to_owned(),
                input: "# T\n\na a a z".to_owned(),
                ids: vec![0, 5, 10, 10, 10, 9, 2],
                canary: false,
            },
        ]
    );
}

#[test]
fn fixtures_of_another_shape_are_refused() {
    for text in [
        "",
        r#"{"fixtures": {"name": "x", "input": ["ab"], "ids": [0, 2]}}"#,
        r#"{"fixtures": [{"name": "x", "input": "ab", "ids": [0, 2]}]}"#,
        r#"{"fixtures": [{"name": "x", "input": ["ab"], "ids": [-1]}]}"#,
        r#"{"fixtures": [{"name": "x", "input": [{"repeat": "a"}], "ids": [0, 2]}]}"#,
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
}

#[test]
fn the_built_in_fixtures_count_every_boundary_in_full() {
    let fixtures = fixtures().unwrap();
    let mut boundaries = Vec::new();
    for fixture in &fixtures {
        // The model adds BOS 0 and EOS 2 to every input.
        assert_eq!(fixture.ids.first(), Some(&0), "{}", fixture.name);
        assert_eq!(fixture.ids.last(), Some(&2), "{}", fixture.name);
        let Some((kind, count)) = fixture.name.split_once('_') else {
            continue;
        };
        if matches!(kind, "plain" | "context") {
            assert_eq!(fixture.ids.len(), count.parse::<usize>().unwrap());
            boundaries.push(fixture.name.as_str());
        }
    }
    // Around the chunk target, 500, the hard maximum, 700, and the model's
    // context, 8192: with and without heading context.
    assert_eq!(
        boundaries,
        [
            "plain_499",
            "plain_500",
            "plain_501",
            "plain_699",
            "plain_700",
            "plain_701",
            "plain_8192",
            "plain_8193",
            "context_499",
            "context_500",
            "context_501",
            "context_699",
            "context_700",
            "context_701",
        ]
    );
    assert_eq!(fixtures.len(), 41);
}
