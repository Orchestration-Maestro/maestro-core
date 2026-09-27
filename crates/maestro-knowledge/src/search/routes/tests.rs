use super::identifier::advances;
use qdrant_client::qdrant::{PointId, point_id::PointIdOptions};

#[test]
fn numeric_and_uuid_scroll_cursors_advance_only_when_greater() {
    let point_id = |point_id_options| PointId {
        point_id_options: Some(point_id_options),
    };
    let number_one = point_id(PointIdOptions::Num(1));
    let number_two = point_id(PointIdOptions::Num(2));
    let uuid_a = point_id(PointIdOptions::Uuid("a".to_owned()));
    let uuid_b = point_id(PointIdOptions::Uuid("b".to_owned()));

    assert!(advances(None, &number_one));
    assert!(advances(Some(&number_one), &number_two));
    assert!(!advances(Some(&number_two), &number_two));
    assert!(!advances(Some(&number_two), &number_one));
    assert!(advances(Some(&uuid_a), &uuid_b));
    assert!(!advances(Some(&uuid_b), &uuid_b));
    assert!(!advances(Some(&number_one), &uuid_b));
    assert!(!advances(Some(&uuid_a), &number_two));
}
