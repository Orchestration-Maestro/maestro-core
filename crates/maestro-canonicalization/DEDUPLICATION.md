# Scoped exact duplicate grouping

**Task #13: PASS for the approved library-only contract. Tests, development gate and independent source review passed. The separate [chunking library](CHUNKING.md) is implemented; Phase B CLI/manifests remain pending.**

## Use the library

`group_exact` operates on already acquired, accepted canonical revisions and their exact original Markdown. It performs no filesystem access or model calls. This synthetic example makes an explicit caller authorization assertion; it is not a permission-discovery recipe:

```rust
use maestro_canonicalization::{
    canonicalize, group_exact, CanonicalizeInput, DedupInput, DedupScope,
    RevisionKey, WarningPolicy,
};

let original = "# Example\n\nDo not delete.\n";
let document = canonicalize(CanonicalizeInput::new(original, "source-a"))?;
let scope = DedupScope {
    tenant_id: "example-tenant".into(),
    workspace_id: "example-workspace".into(),
    authorized_revisions: [RevisionKey {
        document_id: document.document_id.clone(),
        revision_id: document.revision_id.clone(),
    }].into_iter().collect(),
};
let result = group_exact(
    &scope,
    &[DedupInput { document: &document, markdown: original }],
    WarningPolicy::Preserve,
)?;
assert_eq!(result.occurrences.len(), 1);
assert_eq!(result.groups.len(), 2); // One original and one canonical singleton.
# Ok::<(), maestro_canonicalization::Error>(())
```

In a real caller, construct the authorized set from a trusted authority for the stated tenant/workspace. Do not treat a document's own metadata or this example's strings as authorization. Unknown source policies stay unknown. `WarningPolicy::Preserve` is an explicit processing choice, not permission to read or publish.

## What is retained and compared

The equality contract defines the exact projection and exclusions.

| Result | Meaning |
| --- | --- |
| `occurrences` | Source/revision-sorted borrowed canonical records and original Markdown, with every original policy, warning, mapping, coordinate and operational field retained. |
| `Original` / `original-utf8/v1` | Entire original UTF-8 bytes. Uses the replay-verified original SHA-256. |
| `Canonical` / `canonical-structured/v1` | Versioned structured content with document-local IDs remapped to ordinals and positional spans excluded from comparison, not from retained records. SHA-256 covers the serialized projection. |
| `groups` | Deterministic scoped equivalence classes with occurrence indices. Only classes with multiple members are duplicates; there is no selected winner. |
| `scoped-exact-dedup/1` | Result/identity policy. Group IDs cover tenant, workspace, representation, representation profile and content hash using JSON tuple serialization and SHA-256. |

Canonical comparison is conservative. It preserves typed content, heading relationships/attributes, code, tables, link destinations, title/language, opaque extra metadata and supplied extractor structured content. It does not establish semantic equivalence. Equivalent content can remain separate when those conservative fields differ.

A matching hash is only a candidate: the implementation compares full representation bytes before joining a group. Unequal bytes under the same candidate hash fail the operation. Identity and permission differences never disappear from occurrence records.

These are **not prepared embedding-input hashes**. Contextual titles, repeated headers, list context and formatting can change final chunk input. The [chunking API](CHUNKING.md) serializes and counts the complete prepared input using the [qualified tokenizer contract](TOKENIZER.md), then fingerprints that exact input separately.

## Refusals and lifecycle

The entire call fails on blank scope, unauthorized revisions, repeated document/revision keys, replay inconsistency, blocking findings, unapproved warnings, unresolved structural references, serialization failure or hash collision. It returns no partial groups. All input revision grants are checked before source replay; errors do not include source text or identifiers.

Repeated keys are refused rather than choosing one of several parser profiles or operational snapshots. Repeating a valid call is deterministic; reordering its distinct inputs yields identical serialized output. Tenant/workspace strings are not normalized.

For revocation, supply the reduced authorized set and corresponding inputs. An old now-unauthorized input causes refusal. A call containing only surviving authorized occurrences preserves those occurrences and excludes absent ones. There is no global cache, reference-count deletion, automatic revision selection or durable authorization registry.

**A historical result is not a current authorization grant.** Reauthorize before using it later. This library neither implements IAM nor prevents a malicious caller from lying about its supplied authorization set. Old source snapshots remain immutable historical evidence, not automatically retrievable content.

## Verification evidence

Logs are under ignored `target/dedup-acceptance/`.

| Check | Observed result |
| --- | --- |
| Initial red | Missing API compile failure, then four executable contract failures against the refusing stub. |
| Focused contract coverage | Eight integration tests plus a forced-collision unit test; 21 structural/content mutation pairs and four metadata/extractor comparisons inside those tests. |
| Complete offline Rust suite | **52 tests + one doctest passed**, exit 0, 1.473 seconds. Existing Phase A tests remain intact. |
| Strict Clippy | `cargo clippy --all-targets --locked --offline -- -D warnings`: exit 0, 1.453 seconds. |
| Development gate | Repository-root `PATH="$PWD/.tools/bin:$PATH" just check`: exit 0, **20.903 seconds**; **1,741 / 1,989 = 87.53% line coverage**, unchanged 80% floor. |

The new tests cover stable grouping without identity/provenance loss, original-versus-canonical distinctions, scope isolation, revocation, explicit warning handling, invalid/tampered input, unknown policies, independent policy revisions and source-coordinate preservation. All examples are synthetic; no corpus grouping was run.

Five deliberate defects were injected into an **ignored disposable crate copy**, never into the reviewed implementation. The tests detected each: removed collision check, bypassed authorization, trusted saved warnings instead of replay, omitted tenant from group IDs, and flattened structure into retrieval text. `mutation-results.json` records the outcomes. These are bounded targeted checks, **not completion of the separate full golden mutation gate**.

One test initially assumed opaque extractor payloads could have no warnings. Existing Phase A correctly emits `extractor_payload_retained`; the test now explicitly preserves and checks that warning, while separately exercising strict acceptance of a clean document. Production validation was not weakened. The spelling hook initially treated an escaped Unicode fixture fragment as an English typo; using the equivalent literal Unicode character fixed it without changing the test input bytes or scanner rules.

Active LSP probes were inconclusive (timeout/silent server), not clean evidence. Compiler/tests, Clippy, rustdoc and the development gate provide executed validation. Independent read-only review returned **OK, no findings**, for the source and tests: workflow `c986ab2c-a079-46b4-b0e6-600e7344ba73`, child `ee22e96d-f0f6-4fdb-867b-d6cb685f38fc`, managed artifact `review/scoped-dedup.md`. The reviewer inspected code; command execution and final acceptance remain parent evidence.

## Remaining boundaries

This synchronous implementation holds comparison bytes in memory and replays each input. No corpus-scale throughput, bounded-RAM guarantee or incremental processing claim is made. Original records are borrowed rather than cloned.

Library acceptance is separate from chunking, manifests, full security/release and hosted CI acceptance. No CLI, dependencies, model inference, embeddings, indexes, corpus rewrites, source deletions, staging or commits were added by this task.

**Next: task #15 — CLI/manifests over the separately verified chunking API.**
