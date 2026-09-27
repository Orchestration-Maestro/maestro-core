//! Visible metadata returned by `knowledge collections`.

use super::support::Home;
use serde_json::json;

#[test]
fn human_listing_prints_only_visible_collection_lines() {
    let home = Home::new();
    home.add_synthetic();
    let result = home.run(&["knowledge", "collections"]);
    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(result.stderr, "");
    assert_eq!(
        result.stdout,
        concat!(
            "synthetic: Synthetic operations handbook in French and English, ",
            "written for public tests (published generation none)\n",
        )
    );
}

#[test]
fn lists_only_visible_collection_metadata_in_the_cli_envelope() {
    let home = Home::new();
    home.add_synthetic();
    let result = home.run(&["--json", "knowledge", "collections"]);
    assert_eq!(
        (result.code, result.stderr.as_str()),
        (Some(0), ""),
        "{result:?}"
    );
    assert_eq!(
        result.json(),
        json!({
            "schema": "maestro-cli/knowledge-collections/1",
            "data": {
                "schema": "maestro-knowledge-collections/1",
                "collections": [{
                    "id": "synthetic",
                    "title": concat!(
                        "Synthetic operations handbook in French and English, ",
                        "written for public tests",
                    ),
                    "published_generation": null,
                }],
            },
            "truncated": false,
            "limit_bytes": 65536,
        })
    );
}
