# Quality-profile fields (C65)

Registered kind: `quality-profile`, descriptor version 1. Files live at
`profiles/quality/<name>.toml`, relative to common, core, team, language or
standard areas. Identity is `quality-profile:<area>/<name>`. This is not an
agent-session profile, a kernel model card or an S1 preference. Session fields
and `profiles/models/` are not accepted by this descriptor.

The normal `maestro-source/2` metadata envelope applies. Unknown fields refuse.

| Field | Shape and meaning |
| --- | --- |
| `name` | Nonempty string equal to the filename's resource name. |
| `subject` | Nonempty string naming the quality subject. |
| `baseline` | Optional exact `quality-profile:` reference declared in `metadata.requires`. A baseline has no baseline; children name it explicitly. Depth beyond one, cycles and other kinds refuse. |
| `gates` | Nonempty unique list of mandatory gate IDs. Children retain every baseline gate and may add gates; removal refuses. C17's additive class accumulates the required inventory. |
| `applicability` | Nonempty unique list describing applicable inputs. |
| `failure_conditions` | Nonempty unique list of failure conditions. |
| `evidence` | Nonempty unique list of required report formats. |
| `bindings` | Table keyed by every gate ID, with no undeclared gate keys. Each record has `state` and optional `reference`, and no other fields. |
| `thresholds` | Strict table containing `floors` and `ceilings`, both metric-to-number tables (possibly empty). |

## Binding declarations are not successful gates

`state = "unresolved"` explicitly records a missing implementation. It must not
have a reference. `state = "bound"` requires an exact typed catalog reference
also declared in `metadata.requires`; the shared checker validates its existence,
layer and dependency closure. No other state, executable or command is admitted.
Neither state confers runtime qualification, and checking launches no tools.
A missing record refuses rather than supplying a passing or default binding.

## Threshold direction and precision

Floors may rise; ceilings may fall. Every inherited metric remains present.
An attempted widening or removal refuses. Values are nonnegative: integers
retain exact i64 precision, and floats must be finite. The numeric type of an
inherited metric is fixed; integer/float changes refuse with the metric and
both types in the diagnostic. C17 and quality profiles share one generic
numeric narrowing helper, with explicit floor/ceiling direction.

A metric appearing in both tables must have the same numeric type and a floor
no higher than its ceiling. Inconsistent intersections refuse. Thresholds are
data, never rule-specific executable code. S1 registry/defaults and the existing
C17/C81b resolution behaviour remain authoritative and unchanged.

## Fixtures

`tests/fixtures/catalog/quality/` contains complete synthetic profile documents.
The source tests mount each technology fixture beside `baseline.toml` and the
existing synthetic common/core catalog, so reference and ownership checks run
through the production checker. Additional hostile neighbours are made by
single-field edits in `source/tests/quality_profile.rs`.
