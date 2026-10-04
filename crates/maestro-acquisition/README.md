# Native acquisition policy

N03 implements strict source-policy declarations and immutable local baseline
resolution. `PolicySource` and `ResourceSource` are replaceable read-only ports;
`DirectFiles` resolves caller-bound files through the same `validate` function as
synthetic catalog adapters. Neither resolver starts transports/sessions, creates
grants, writes a bundle or activates connectors.

`maestro-collection/1` still imports. `maestro-collection/2` requires
`source_policy`, explicitly null for import-only collections or an exact ID and
SHA-256 digest. v1 refuses the added field. Every referenced resource requires
separate reviewed admission evidence binding its exact digest and host platform;
adapter capabilities and extraction-profile member refs are checked before a
`CheckedPolicy` can be returned. Read grants come from the kernel, not JSON.

The typed policy schemas derive JSON Schema with `schemars::schema_for!`.
`parse_policy` and `policy::resolve::parse_resource` share the bounded strict JSON
parser with collection declarations: 4 MiB, 32 container levels, 20,000 cumulative
members/elements, 10,000 elements per array and 1,000 sources/acquisition profiles.
Nullable fields must be present. Readiness uses literal direct-child DOM paths,
never CSS, scripts or callbacks, and is bounded by source/aggregate deadlines.

Qualification evidence, installed adapter declarations and extraction-profile
internals remain immutable references with trusted admission summaries. N15/N16
own registry/detector/extraction schemas, N43/N46 adapter execution and N47 wiki
execution. N30 owns manifest overlays/CAS. No live qualification, network, browser,
extraction or publication is implemented or claimed here.

Run the independently authored synthetic contracts:

```sh
CARGO_BUILD_JOBS=3 capped cargo test -p maestro-acquisition --locked n03_
```
