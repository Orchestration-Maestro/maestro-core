//! Every new Ref position remains an object, never a positional alias.
use super::{n30_support::Fixture, n57_processing_artifacts::initial};
use maestro_acquisition::{
    adaptation::snapshot::ProcessingSnapshot, extraction::model::Processing,
    policy::resolve::parse_resource,
};
use serde_json::{Value, json};

fn array(reference: &Value) -> Value {
    json!([reference["id"], reference["digest"]])
}
#[test]
fn n57_processing_pin_fields_require_objects() {
    let fixture = Fixture::new();
    let snapshot = initial(&fixture);
    let processing = &snapshot.effective.sources["notes"].2;
    for field in ["cleanup", "chunk", "dedup"] {
        let mut value = json!(processing);
        value[field] = array(&value[field]);
        assert!(
            parse_resource::<Processing>(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{field}"
        );
    }
}
#[test]
fn n57_effective_pin_maps_sources_and_nullable_slots_require_objects() {
    let fixture = Fixture::new();
    let snapshot = initial(&fixture);
    for position in ["map", "source", "nullable"] {
        let mut value = json!(snapshot);
        match position {
            "map" => {
                let reference = value["effective"]["approved_cleanup"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap();
                *reference = array(reference);
            }
            "source" => {
                let reference = &mut value["effective"]["sources"]["notes"][0];
                *reference = array(reference);
            }
            _ => {
                value["effective"]["selected"][0] =
                    array(&value["effective"]["sources"]["notes"][2]["cleanup"]);
            }
        }
        assert!(
            parse_resource::<ProcessingSnapshot>(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{position}"
        );
    }
}
