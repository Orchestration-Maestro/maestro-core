use super::super::versions::compare_numeric_versions;
use std::cmp::Ordering;

#[test]
fn numeric_versions_compare_components_without_machine_integer_limits() {
    assert_eq!(
        compare_numeric_versions("2.10", "2.9"),
        Some(Ordering::Greater)
    );
    assert_eq!(compare_numeric_versions("2.0", "2"), Some(Ordering::Equal));
    assert_eq!(
        compare_numeric_versions("02.0003", "2.3.0"),
        Some(Ordering::Equal)
    );
    assert_eq!(
        compare_numeric_versions("1.9", "1.10"),
        Some(Ordering::Less)
    );
    assert_eq!(
        compare_numeric_versions(
            &format!("1.{}", "9".repeat(100)),
            &format!("1.{}", "1".repeat(101))
        ),
        Some(Ordering::Less)
    );
}

#[test]
fn non_numeric_or_malformed_versions_have_no_order() {
    for version in ["", ".1", "1.", "1..2", "v1.2", "1.-2", " 1.2"] {
        assert_eq!(
            compare_numeric_versions(version, "1.2"),
            None,
            "{version:?}"
        );
    }
}
