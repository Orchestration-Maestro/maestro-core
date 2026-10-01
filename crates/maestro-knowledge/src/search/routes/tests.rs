use super::identifier_cursor::advances;
use crate::index::ProjectionCursor;

#[test]
fn numeric_and_uuid_scroll_cursors_advance_only_when_greater() {
    let number_one = ProjectionCursor::Number(1);
    let number_two = ProjectionCursor::Number(2);
    let uuid_a = ProjectionCursor::Text("a".to_owned());
    let uuid_b = ProjectionCursor::Text("b".to_owned());

    assert!(advances(None, &number_one));
    assert!(advances(Some(&number_one), &number_two));
    assert!(!advances(Some(&number_two), &number_two));
    assert!(!advances(Some(&number_two), &number_one));
    assert!(advances(Some(&uuid_a), &uuid_b));
    assert!(!advances(Some(&uuid_b), &uuid_b));
    assert!(!advances(Some(&number_one), &uuid_b));
    assert!(!advances(Some(&uuid_a), &number_two));
}
