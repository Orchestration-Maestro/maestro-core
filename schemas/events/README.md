# Event schemas

The JSON Schema of each public event's data
([07 §3.2](../../docs/architecture/07-extensibility.md#32-event-catalogue)),
generated from its Rust type in
[`journal/knowledge.rs`](../../crates/maestro-kernel/src/journal/knowledge.rs).
An event's `dataschema` names its file: `maestro://schemas/events/<name>/<major>`
is `<name>/<major>.json` here.

| Event type | Schema of its data |
| --- | --- |
| `maestro.knowledge.import.completed.v1` | [knowledge.import.completed/1.json](knowledge.import.completed/1.json) |
| `maestro.knowledge.revision.held.v1` | [knowledge.revision.held/1.json](knowledge.revision.held/1.json) |
| `maestro.knowledge.generation.published.v1` | [knowledge.generation.published/1.json](knowledge.generation.published/1.json) |
| `maestro.knowledge.generation.retired.v1` | [knowledge.generation.retired/1.json](knowledge.generation.retired/1.json) |

## Changing a schema

Within a major version a schema only grows, by an optional field. The tests
of `maestro-kernel` fail when a type removes or narrows anything its
committed schema held (a property gone, a type narrowed, a new required
property, a value gone), and when a committed schema is not the one its type
generates. After an additive change, this command regenerates the files:

```sh
cargo test -p maestro-kernel --locked -- --ignored regenerate_the_committed_schemas
```

It writes nothing when a type narrows its schema. A narrowing is a new major
version: a new type, `.v2`, and a new file, `<name>/2.json`, beside the old
ones.
