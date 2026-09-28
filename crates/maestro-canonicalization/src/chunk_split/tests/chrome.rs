//! Page chrome under the complete-ideas profile: only a label or markup of known HTML elements
//! leaves the indexed text; any other content stays.
use super::ideas::{idea_chunks, prepared};

#[test]
fn a_paragraph_of_an_xml_element_with_attributes_stays_indexed() {
    let markdown = "# Variables\n\nSet the variable:\n\n<setting name=\"a\" value=\"b\"/>\n";
    let texts = prepared(markdown);
    assert!(
        texts
            .iter()
            .any(|text| text.contains("<setting name=\"a\" value=\"b\"/>")),
        "{texts:?}"
    );
}

#[test]
fn a_paragraph_of_a_placeholder_element_stays_indexed() {
    let markdown = "# Paths\n\nReplace the placeholder:\n\n<name>\n";
    let texts = prepared(markdown);
    assert!(
        texts.iter().any(|text| text.contains("<name>")),
        "{texts:?}"
    );
}

#[test]
fn closed_as_an_item_of_a_list_of_states_stays_indexed() {
    let texts = prepared("- Open\n\n- Closed\n");
    assert!(
        texts.iter().any(|text| text.contains("Closed")),
        "{texts:?}"
    );
}

#[test]
fn closed_before_an_image_placeholder_leaves_the_indexed_text_with_it() {
    assert_eq!(
        prepared("# States\n\nClosed\n\n<!-- image -->\n\nThe job ended.\n"),
        ["States\n\nThe job ended."]
    );
}

#[test]
fn a_copy_label_ending_a_paragraph_leaves_the_indexed_text() {
    let markdown = "# Example\n\nRun the command below. Copy Copied to clipboard\n";
    let (mapped, chunks) = idea_chunks(markdown).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(
        chunks[0].prepared_input,
        "Example\n\nRun the command below."
    );
    let fragment = chunks[0].fragments.last().unwrap();
    let unit = &mapped.units[fragment.contribution.unit_index];
    let range = fragment.contribution.range;
    assert_eq!(&unit.text[range.start..range.end], "Run the command below.");
}

#[test]
fn a_copy_label_inside_a_table_cell_stays() {
    let markdown = "| Code |\n|---|\n| run it Copy Copied to clipboard |\n";
    let texts = prepared(markdown);
    assert!(
        texts
            .iter()
            .any(|text| text.contains("run it Copy Copied to clipboard")),
        "{texts:?}"
    );
}

#[test]
fn two_small_tables_of_one_width_in_one_section_keep_their_own_chunks() {
    let markdown = "# Keys\n\n| Key | Value |\n|---|---|\n| one | 1 |\n\n\
                    | Key | Value |\n|---|---|\n| two | 2 |\n";
    assert_eq!(
        prepared(markdown),
        ["Keys\n\nKey\tValue\none\t1", "Keys\n\nKey\tValue\ntwo\t2"]
    );
}
