//! Point IDs: the `UUIDv5` of the chunk's ID, in the namespace that is the
//! `UUIDv5` of the URL of point IDs, pinned by what Python's `uuid` computes:
//! `uuid5(uuid5(NAMESPACE_URL,
//! "https://github.com/Orchestration-Maestro/maestro-core#qdrant-point"),
//! chunk)`, whose namespace is `8cfb99af-c40a-5a27-a068-e1e297c912d2`.

use super::super::point::point_id;

#[test]
fn a_point_id_is_the_uuid_v5_of_its_chunks_id() {
    for (chunk, point) in [
        ("chunk-a", "4b73a1bf-d4cc-5371-bed8-93b30bebe501"),
        ("chunk-1f0e", "1ccfc201-f005-58a4-92e4-e41d3979c01b"),
        ("", "84f1d796-fa35-515f-a252-85be135387af"),
    ] {
        assert_eq!(point_id(chunk), point, "{chunk:?}");
    }
}
