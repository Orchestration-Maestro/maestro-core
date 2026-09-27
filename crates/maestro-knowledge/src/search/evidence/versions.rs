//! Numeric release-label ordering.

use std::cmp::Ordering;

/// Compares dot-separated ASCII integer components without integer overflow.
pub(crate) fn compare_numeric_versions(left: &str, right: &str) -> Option<Ordering> {
    let left = components(left)?;
    let right = components(right)?;
    for index in 0..left.len().max(right.len()) {
        let left_component = left.get(index).copied().unwrap_or("0");
        let right_component = right.get(index).copied().unwrap_or("0");
        let order = compare_component(left_component, right_component);
        if order != Ordering::Equal {
            return Some(order);
        }
    }
    Some(Ordering::Equal)
}

/// Splits a valid nonempty dot-separated numeric version.
fn components(version: &str) -> Option<Vec<&str>> {
    if version.is_empty() {
        return None;
    }
    let components: Vec<_> = version.split('.').collect();
    components
        .iter()
        .all(|component| {
            !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
        })
        .then_some(components)
}

/// Compares components by normalized decimal length and then digits.
fn compare_component(left: &str, right: &str) -> Ordering {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    let left = if left.is_empty() { "0" } else { left };
    let right = if right.is_empty() { "0" } else { right };
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}
