# Event schemas

The JSON Schema of each public event's data
([07 §3.2](../../docs/architecture/07-extensibility.md#32-event-catalogue)),
generated from its Rust type in
[`journal/knowledge.rs`](../../crates/maestro-kernel/src/journal/knowledge.rs).
An event's `dataschema` names its file: `maestro://schemas/events/<name>/<major>`
is `<name>/<major>.json` here. [index.json](index.json) lists every committed
schema by name.

| Event type | Schema of its data |
| --- | --- |
| `maestro.knowledge.import.completed.v1` | [knowledge.import.completed/1.json](knowledge.import.completed/1.json) |
| `maestro.knowledge.revision.held.v1` | [knowledge.revision.held/1.json](knowledge.revision.held/1.json) |
| `maestro.knowledge.generation.published.v1` | [knowledge.generation.published/1.json](knowledge.generation.published/1.json) |
| `maestro.knowledge.generation.retired.v1` | [knowledge.generation.retired/1.json](knowledge.generation.retired/1.json) |

## Changing a schema

Within a major version a schema only gains optional properties. The tests
of `maestro-kernel` fail when a type changes anything else its committed
schema held (descriptions aside): a property gone, a new required property,
a type, a value, a format or a bound changed, whether it narrows or widens.
They also fail when a committed schema is not the one its type generates,
and when the index does not list exactly the schemas of the public events.
After an additive change, this command regenerates the files:

```sh
cargo test -p maestro-kernel --locked -- --ignored regenerate_the_committed_schemas
```

It writes nothing when a type breaks its schema, when a schema the index
lists has lost its file, or when the index lists a schema no public event
has. A new public event's schema joins the index the first time the command
writes it. A breaking change is a new major version: a new type, `.v2`, and
a new file, `<name>/2.json`, beside the old ones.

The command guards its own use only: deleting a schema and editing the
index together still get past it. The full guard is a CI step, still to
come, that compares each schema with the one committed on `main`, its
released predecessor (FR-S1-008b).
