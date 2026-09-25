# Architecture decision records

Hard-to-reverse decisions, each with the trade-off that produced it. Numbered in
order; a superseded record stays and names its successor.

| ADR | Decision |
| --- | --- |
| [0001](0001-fresh-start-with-canonicalization-only.md) | Fresh start: only the canonicalization crate is carried over |
| [0002](0002-one-authority-many-projections.md) | SQLite + content-addressed artifacts are the only authority; indexes and graphs are projections |
| [0003](0003-qdrant-server-for-search-projections.md) | Qdrant server for vector and lexical projections |
| [0004](0004-neo4j-for-the-graph-projection.md) | Neo4j Community for the graph projection; LadybugDB under evaluation |
| [0005](0005-copilot-native-catalog-formats.md) | Copilot-native formats are the canonical catalog formats |
| [0006](0006-in-house-event-sourced-workflow-engine.md) | An in-house, event-sourced workflow engine on the kernel journal |
| [0007](0007-cedar-for-authorization.md) | Cedar for tool and operation authorization |
| [0008](0008-chunks-budgeted-in-embedder-tokens.md) | Chunks are budgeted in the selected embedder's tokens |
| [0009](0009-vendor-specific-material-stays-private.md) | Vendor-specific connectors, corpora and questions stay private |
| [0010](0010-spec-kit-and-executable-gates.md) | Spec Kit for delivery; evaluations and tests are the gates |
| [0011](0011-models-chosen-by-bake-off.md) | No model is preselected; every role is filled by a recorded bake-off |
| [0012](0012-catalog-and-runtime-in-separate-repositories.md) | Catalog content and runtime live in separate repositories |
| [0013](0013-extensions-through-events-and-operations-out-of-process.md) | Extensions plug in through versioned events and operations, out of process |
| [0014](0014-strict-json-for-collection-and-source-policy.md) | Collection and source-policy declarations are strict JSON |
| [0015](0015-bundle-freshness-and-revocation.md) | Bundles need freshness and revocation, not only signatures |
| [0016](0016-native-rust-desktop-workbench.md) | The workbench is a native Rust desktop application |
| [0017](0017-spec-kit-installed-once-for-the-organization.md) | Spec Kit is installed once for the organization, not committed |
| [0018](0018-rustix-on-unix-and-win32-flags-on-windows.md) | The snapshot store uses rustix on Unix and the standard library's Win32 flags on Windows |
| [0019](0019-reverse-engineering-is-analysis-behind-a-clean-room.md) | Reverse engineering produces knowledge only, behind a clean-room boundary |
