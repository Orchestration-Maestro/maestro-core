//! `config set` and `config unset` on a file's text: only the value's bytes
//! or the key's lines change, comments and order stay, every result is
//! parsed strictly before it is returned, and a form that cannot be edited
//! safely is refused.

use crate::{Registry, Value, set_in_document, unset_in_document};

/// The file's first line.
const SCHEMA: &str = "schema = \"maestro-preferences/1\"\n";

/// `text` with `key` set to the command line's `value`, or the refusal.
fn set(text: Option<&str>, key: &str, value: &str) -> Result<String, String> {
    let registry = Registry::built_in().unwrap();
    let value = registry.get(key).unwrap().kind.parse_text(value).unwrap();
    set_in_document(&registry, text, key, &value).map_err(|error| error.to_string())
}

/// `text` without `key`, `None` when it does not set it, or the refusal.
fn unset(text: &str, key: &str) -> Result<Option<String>, String> {
    let registry = Registry::built_in().unwrap();
    unset_in_document(&registry, text, key).map_err(|error| error.to_string())
}

#[test]
fn set_in_document_writes_a_new_file_from_its_schema() {
    assert_eq!(
        set(None, "tone", "brief"),
        Ok(format!("{SCHEMA}tone = \"brief\"\n"))
    );
    assert_eq!(
        set(None, "search.rerank.depth", "40"),
        Ok(format!("{SCHEMA}\n[search.rerank]\ndepth = 40\n"))
    );
}

#[test]
fn set_in_document_replaces_only_the_value_bytes_wherever_the_value_is() {
    let text = format!(
        "# My preferences\n{SCHEMA}tone = \"brief\" # short\n\n[search]\n# passages\nk = 20\n\
         weights = {{ dense = 1.0, lexical = 2 }}\nrerank.depth = 40\n\
         section_prior.classes = [\n  \"changelog\", # old\n]\n"
    );
    assert_eq!(
        set(Some(&text), "tone", "detailed").unwrap(),
        text.replace("tone = \"brief\"", "tone = \"detailed\"")
    );
    assert_eq!(
        set(Some(&text), "search.k", "7").unwrap(),
        text.replace("k = 20", "k = 7")
    );
    assert_eq!(
        set(Some(&text), "search.weights.lexical", "0.5").unwrap(),
        text.replace("lexical = 2", "lexical = 0.5")
    );
    assert_eq!(
        set(Some(&text), "search.rerank.depth", "12").unwrap(),
        text.replace("depth = 40", "depth = 12")
    );
    assert_eq!(
        set(Some(&text), "search.section_prior.classes", "conversion").unwrap(),
        text.replace("[\n  \"changelog\", # old\n]", "[\"conversion\"]")
    );
    let multiline = format!("{SCHEMA}tone = \"\"\"\nbrief\"\"\"\n");
    assert_eq!(
        set(Some(&multiline), "tone", "normal"),
        Ok(format!("{SCHEMA}tone = \"normal\"\n"))
    );
}

#[test]
fn set_in_document_adds_a_key_to_its_table_after_the_table_last_line() {
    let text = format!(
        "{SCHEMA}language = \"fr\"\n\n# Search\n[search]\nk = 20 # mine\n\n\
         [search.rerank]\ndepth = 40\n"
    );
    assert_eq!(
        set(Some(&text), "search.max_tokens", "5000"),
        Ok(text.replace("k = 20 # mine\n", "k = 20 # mine\nmax_tokens = 5000\n"))
    );
    assert_eq!(
        set(Some(&text), "search.routes.dense", "false"),
        Ok(text.replace("k = 20 # mine\n", "k = 20 # mine\nroutes.dense = false\n"))
    );
    assert_eq!(
        set(Some(&text), "search.rerank.blend", "0.3"),
        Ok(format!("{text}blend = 0.3\n"))
    );
    assert_eq!(
        set(Some(&text), "tone", "brief"),
        Ok(text.replace(
            "language = \"fr\"\n",
            "language = \"fr\"\ntone = \"brief\"\n"
        ))
    );
    assert_eq!(
        set(Some(&text), "ask.k", "3"),
        Ok(format!("{text}\n[ask]\nk = 3\n"))
    );
    let empty_table = format!("{SCHEMA}[search]\n\n[ask]\n");
    assert_eq!(
        set(Some(&empty_table), "search.k", "3"),
        Ok(format!("{SCHEMA}[search]\nk = 3\n\n[ask]\n"))
    );
}

#[test]
fn set_in_document_adds_a_key_beside_its_dotted_siblings() {
    let text = format!("{SCHEMA}search.k = 20\n  search.rerank.depth = 40\n\n[ask]\nk = 3\n");
    assert_eq!(
        set(Some(&text), "search.max_tokens", "5000"),
        Ok(text.replace(
            "search.k = 20\n",
            "search.k = 20\nsearch.max_tokens = 5000\n"
        ))
    );
    assert_eq!(
        set(Some(&text), "search.rerank.blend", "0.5"),
        Ok(text.replace(
            "  search.rerank.depth = 40\n",
            "  search.rerank.depth = 40\n  search.rerank.blend = 0.5\n"
        ))
    );
}

#[test]
fn set_in_document_keeps_crlf_line_endings_and_a_missing_last_newline() {
    let crlf = "schema = \"maestro-preferences/1\"\r\n[search]\r\nk = 20\r\n";
    assert_eq!(
        set(Some(crlf), "search.max_tokens", "5000"),
        Ok(
            "schema = \"maestro-preferences/1\"\r\n[search]\r\nk = 20\r\nmax_tokens = 5000\r\n"
                .to_owned()
        )
    );
    assert_eq!(
        set(Some(crlf), "ask.k", "2"),
        Ok(format!("{crlf}\r\n[ask]\r\nk = 2\r\n"))
    );
    let unterminated = "schema = \"maestro-preferences/1\"\n[search]\nk = 20";
    assert_eq!(
        set(Some(unterminated), "search.max_tokens", "5000"),
        Ok(format!("{unterminated}\nmax_tokens = 5000"))
    );
    assert_eq!(
        set(Some(unterminated), "ask.k", "2"),
        Ok(format!("{unterminated}\n\n[ask]\nk = 2\n"))
    );
    let bare = "schema = \"maestro-preferences/1\"";
    assert_eq!(
        set(Some(bare), "tone", "brief"),
        Ok(format!("{bare}\ntone = \"brief\""))
    );
}

#[test]
fn set_in_document_refuses_to_add_a_key_inside_an_inline_table() {
    for (text, key) in [
        (
            format!("{SCHEMA}[search]\nweights = {{ dense = 1.0 }}\n"),
            "search.weights.lexical",
        ),
        (
            format!("{SCHEMA}search = {{ rerank.depth = 3 }}\n"),
            "search.rerank.blend",
        ),
    ] {
        assert_eq!(
            set(Some(&text), key, "0.5"),
            Err(format!(
                "{key} cannot be edited safely in this file's layout (an inline table or an \
                 unusual form); edit the file by hand"
            )),
            "{text}"
        );
    }
}

#[test]
fn set_in_document_refuses_a_file_it_cannot_read() {
    assert_eq!(
        set(
            Some(&format!("{SCHEMA}[access]\nread = []\n")),
            "tone",
            "brief"
        ),
        Err("unknown key \"access\"".to_owned())
    );
    assert_eq!(
        set(Some("tone = \"brief\"\n"), "tone", "normal"),
        Err("the file has no schema = \"maestro-preferences/1\" line".to_owned())
    );
}

#[test]
fn unset_in_document_removes_the_key_lines_and_nothing_else() {
    let text = format!(
        "# Mine\n{SCHEMA}\n# How long\ntone = \"brief\" # short\nlanguage = \"fr\"\n\n[search]\n\
         k = 20\nsection_prior.classes = [\n  \"changelog\",\n]\nmax_tokens = 5000"
    );
    assert_eq!(
        unset(&text, "tone"),
        Ok(Some(text.replace("tone = \"brief\" # short\n", "")))
    );
    assert_eq!(
        unset(&text, "search.section_prior.classes"),
        Ok(Some(text.replace(
            "section_prior.classes = [\n  \"changelog\",\n]\n",
            ""
        )))
    );
    assert_eq!(
        unset(&text, "search.max_tokens"),
        Ok(Some(text.replace("max_tokens = 5000", "")))
    );
    assert_eq!(unset(&text, "search.rerank.depth"), Ok(None));
    let crlf = "schema = \"maestro-preferences/1\"\r\ntone = \"brief\"\r\nlanguage = \"fr\"\r\n";
    assert_eq!(
        unset(crlf, "tone"),
        Ok(Some(
            "schema = \"maestro-preferences/1\"\r\nlanguage = \"fr\"\r\n".to_owned()
        ))
    );
}

#[test]
fn unset_in_document_refuses_a_key_sharing_an_inline_table() {
    let text = format!("{SCHEMA}[search]\nweights = {{ dense = 1.0, lexical = 2.0 }}\n");
    assert_eq!(
        unset(&text, "search.weights.dense"),
        Err(
            "search.weights.dense cannot be edited safely in this file's layout \
             (an inline table or an unusual form); edit the file by hand"
                .to_owned()
        )
    );
    let alone = format!("{SCHEMA}[search]\nweights = {{ dense = 1.0 }}\nk = 3\n");
    assert_eq!(
        unset(&alone, "search.weights.dense"),
        Ok(Some(format!("{SCHEMA}[search]\nk = 3\n")))
    );
}

#[test]
fn set_then_unset_reads_back_as_the_value_then_the_file() {
    let registry = Registry::built_in().unwrap();
    let text = format!("{SCHEMA}[search]\nk = 20\n");
    let changed = set(Some(&text), "search.rerank.blend", "0.25").unwrap();
    let layer = crate::Layer::parse(&registry, &changed).unwrap();
    assert_eq!(layer.get("search.rerank.blend"), Some(&Value::Number(0.25)));
    assert_eq!(unset(&changed, "search.rerank.blend"), Ok(Some(text)));
}

#[test]
fn set_and_unset_refuse_a_file_that_sets_a_setting_twice() {
    let twice = format!("{SCHEMA}\"search.k\" = 3\n[search]\nk = 9\n");
    let refused = "search.k: the setting is set twice in the file".to_owned();
    assert_eq!(set(Some(&twice), "search.k", "4"), Err(refused.clone()));
    assert_eq!(set(Some(&twice), "tone", "brief"), Err(refused.clone()));
    assert_eq!(unset(&twice, "search.k"), Err(refused));
}
