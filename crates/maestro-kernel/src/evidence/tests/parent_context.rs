use super::support::bundle;
use crate::evidence::Bundle;
use serde_json::json;

#[test]
fn parent_context_rejects_duplicate_cross_revision_and_dual_relations_on_read_and_write() {
    let mut valid = bundle();
    valid.trace[0].chunk_ids = vec!["primary".to_owned()];
    valid.trace[1].parent_context_of = vec!["primary".to_owned()];
    let encoded = serde_json::to_value(&valid).unwrap();
    assert_eq!(
        serde_json::from_value::<Bundle>(encoded.clone()).unwrap(),
        valid
    );
    for case in ["duplicate", "cross-revision", "dual-relation"] {
        let mut bad = valid.clone();
        let mut value = encoded.clone();
        match case {
            "duplicate" => {
                bad.trace[1].parent_context_of.push("primary".to_owned());
                value["trace"][1]["parent_context_of"] = json!(["primary", "primary"]);
            }
            "cross-revision" => {
                bad.passages[1].revision_id = "other-revision".to_owned();
                value["passages"][1]["revision_id"] = json!("other-revision");
                let mut without_context = bad.clone();
                without_context.trace[1].parent_context_of.clear();
                assert!(serde_json::to_vec(&without_context).is_ok());
            }
            _ => {
                bad.trace[1].chunk_ids = vec!["primary".to_owned()];
                value["trace"][1]["chunk_ids"] = json!(["primary"]);
            }
        }
        assert!(serde_json::to_vec(&bad).is_err(), "write {case}");
        assert!(
            serde_json::from_value::<Bundle>(value).is_err(),
            "read {case}"
        );
    }
}
