# Unit graph wire contract v1

B06 writes this contract; B07 owns independent kernel DTOs; B10 reads scoped graphs.
The golden files are `crates/maestro-kernel/tests/fixtures/unit-graph-v1.json`,
`unit-mapping-v1.json` and the original synthetic `unit-graph-v1.txt` beside them.
The fixture is deliberately small; it is not a ranking-quality example. Its source
text is illustrative and not parsed as Markdown; producer conformance is tested on
B06's built snapshot, which B08 makes the single shared golden. Repository JSON
fixture files end in one LF for text-file hygiene; tests remove exactly that LF
before treating them as CAS bytes. Runtime CAS bytes have no LF.

## Bytes and identity

Objects serialize in the exact field order shown in the fixtures. Arrays are
ordered, never sets serialized in insertion order. JSON is compact UTF-8, without
BOM, trailing newline or whitespace outside strings. Numbers are nonnegative
integers. Unknown fields and duplicate object keys are refused. Digests are
64 lowercase SHA-256 hexadecimal characters, without a prefix. IDs are opaque
nonempty strings, stable under insertion-order changes. All source ranges are
nonempty half-open original Markdown byte offsets on UTF-8 boundaries.

The graph CAS digest hashes the entire graph JSON. It is external: there is no
`graph_digest` field inside the graph. The descriptor's `mapping_digest` hashes
the entire separate mapping JSON. Both artifacts bind the original Markdown
digest. Profile and preparation digests bind the caller's complete rules.

## Graph, parts and mapping

Graph fields, in order, are `descriptor`, `parts`, `units`, `exclusions`,
`retrieval_views`, and `groups`. Descriptor field order and all
nested object field order are pinned by the fixture. Every field is required;
optional identities use explicit null, never omitted fields.

A part is the sole owner of its source `ranges` and canonical `mappings`.
Parts are sorted by `(first range.start, part_id)`, and IDs are unique. Units,
groups, and memberships reference part IDs only; they never repeat ranges,
mappings, or roles. Unit `part_ids` contain all primary parts in source order.
A group's `part_ids` contain only its direct parts: a heading or a part named by
one of that group's typed context relations. A part ID may occur in many nodes,
but its ranges and mappings exist once in the top-level `parts` array.

Units are sorted by `(first primary part start, unit_id)`. Groups are sorted by
`(first descendant part start, group_id)`. Each node's part IDs are unique and
in source order. Views are sorted by `(first primary part of first membership,
retrieval_view_id)`; memberships are ordered by delivery-unit source order.
Each membership has nonempty primary part IDs, in that unit's source order and
as a subset of the unit's primary part IDs. A view may reference multiple units
and disjoint ranges. No other source-order array is normalized by the reader;
noncanonical order is rejected.

Every non-excluded part is referenced by exactly one unit's ordered `part_ids`,
a group `heading`, or a typed context relation; unit ownership is derived, not
serialized separately. A heading part must be a page or section `heading` and
must never be owned by a delivery unit. Context-relation parts may also be
unit-owned. Ranges and mappings exist once in `parts`, and the complete set of
part ranges plus exclusions partitions the original source bytes. Exclusions
are source-ordered ineligible ranges with nonempty reasons and are identical in
graph and mapping artifacts; headings are never exclusions. Mapping fields are `schema_version`
(`maestro-unit-mapping/1`), `original_markdown_digest`, `contributions`, and
`exclusions`. Each contribution has `range` and `mapping`; the latter has
canonical `unit_id`, `derived_range` (two integer byte offsets), and
`mapping_mode` (nonempty canonical mapping name). Contributions follow source
order. Each primary part's ranges and mappings pair by ordinal, one to one, and
must equal ledger entries. Eligible ranges and exclusions partition the original
source bytes.

## Units, groups, context and searchability

Groups carry ordered direct children and reciprocal parent links, without cycles.
Kinds are `row`, `table`, `procedure`, `section`, `page`, `paragraphs`, and
`code`. Unit kinds additionally include `code` and `block` (no `page`). A row
group's parent is a table; a table's parent is a section or page; a section's
parent is a section or page; a page has no parent. A procedure or code group
has a section or page parent. A code group may directly own one lead-in part
through `lead_in`; the relation is optional when no adjacent lead-in exists. The
code-block unit is always one of its children. A unit's parent, if
present, is a group and the group's children name the unit.

Sections and pages may name an optional `heading` part, which must be their
first part. A heading is not a context relation. Prepared retrieval text may
derive heading paths from ancestor heading parts; required context never
includes headings because the bundle carries section paths.

Context relations are inside each group and have `kind` (`header_to_table`,
`caption_to_table`, or `lead_in`) and `part_id`; array position is the ordinal.
A relation's part is directly owned by that group. Header/caption relations
belong only to table groups. `lead_in` belongs only to procedure, code, or
section groups. Every direct group part is either its heading or typed context;
other unsearchable direct content is refused. A unit's `required_context` is
derived: collect the ordered context-relation parts of its ancestor groups,
exclude headings, sort by source order and deduplicate. Units and memberships
do not serialize that derived list. Membership reads expose primary IDs and
this derived context for B10.

Every unit-owned primary part must occur in a retrieval membership or be a
typed context relation part; heading parts are not unit-owned. Continuation is
`{"kind":"continuation","ordinal":0,"total":2}` with `total > 1` and
`ordinal < total`; otherwise split is `{"kind":"whole"}`. Families use the
explicit structural fields shown in the fixture; no metadata version inference.
An unavailable namespace must be replaced by a caller's explicit document-local
namespace, never an empty cross-document key.

## Golden CAS digests

- Graph: `f1a6562264b5d0d406204c7c57b43a61babbeb36e8416e1386ef3edc012d3664`
- Mapping: `de4b7353da814a92f84c8ffcf8cd03a055d7dc8d770e62bc09b282a2431bb4dd`

The kernel rejects noncanonical serialization rather than silently hashing a
normalized replacement. No graph/group/family authorizes a source or expansion;
B10 obtains each candidate graph through the source-scoped database operation.
