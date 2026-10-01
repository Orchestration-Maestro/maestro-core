//! N57 canonical bytes require explicit nulls without changing S1 card parsing.
use super::{
    n30_support::Fixture,
    n57_processing_artifacts::{check, initial},
    n57_support as support,
};
use maestro_acquisition::adaptation::{artifacts::S1ChunkStrategy, storage};

#[test]
fn n57_artifacts_require_exact_typed_bytes_and_explicit_null() {
    for spelling in ["explicit-null", "omitted-null", "whitespace", "reordered"] {
        let mut fixture = Fixture::new();
        let mut snapshot = initial(&fixture);
        let old = &snapshot.effective.sources["notes"].2.chunk;
        let mut strategy: S1ChunkStrategy =
            storage::artifact(&fixture.db, "synthetic-reader", old).unwrap();
        strategy.resource.id = "new-strategy".into();
        let bytes = serde_json::to_string(&strategy).unwrap();
        let bytes = match spelling {
            "omitted-null" => bytes.replace(",\"output_tokens\":null", ""),
            "whitespace" => format!(" {bytes}"),
            "reordered" => bytes.replace(
                "\"context_tokens\":4096,\"output_tokens\":null",
                "\"output_tokens\":null,\"context_tokens\":4096",
            ),
            _ => bytes,
        };
        let pin =
            storage::retain(&fixture.db, &[support::TAG.into()], bytes.as_bytes(), &[]).unwrap();
        fixture
            .catalog
            .0
            .get_mut("processing-qualification")
            .unwrap()
            .admission
            .references
            .push(pin.clone());
        snapshot
            .effective
            .approved_chunks
            .insert(pin.id.clone(), pin.clone());
        snapshot
            .effective
            .qualified_chunk_tokens
            .insert(pin.id.clone(), 700);
        snapshot.effective.selected.1 = Some(pin.clone());
        for (_, _, processing) in snapshot.effective.sources.values_mut() {
            processing.chunk = pin.clone();
        }
        assert_eq!(
            check(&fixture, &snapshot),
            spelling == "explicit-null",
            "{spelling}"
        );
    }
}
