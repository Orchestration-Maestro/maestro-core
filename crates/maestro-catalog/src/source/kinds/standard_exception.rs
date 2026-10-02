//! Central records live only in root or standard-owned exception placements.

use super::standard_check;
use crate::source::descriptor::{Field, FieldType, KindDescriptor, Scope};

/// The one exception shape; local placements are not registered.
pub(super) fn descriptor() -> KindDescriptor {
    let mut descriptor = standard_check::descriptor();
    "standard-exception".clone_into(&mut descriptor.kind);
    "exceptions".clone_into(&mut descriptor.directory);
    descriptor.scopes = vec![Scope::Root, Scope::Standard];
    descriptor.fields = vec![
        Field::required("name", FieldType::Text),
        Field::required("rule", FieldType::Text),
        Field::required("scopes", FieldType::TextList),
        Field::required("rationale", FieldType::Text),
        Field::required("expiry", FieldType::Text),
        Field::required("evidence", FieldType::Text),
    ];
    descriptor.hook = Some("standard-exception".to_owned());
    descriptor
}
