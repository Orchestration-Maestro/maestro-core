# Preview and manually capture a public source

Start with a reviewed `maestro-collection/2` manifest, its exact source-owned
policy and resources, and separate machine-local bindings. A manifest does not
grant you permission to fetch. The owner must grant each exact source, collection,
public account and target through the [authority service](acquisition-authority.md).
Real public sources still require an OA4a grant. No live grant is included here.

This MVP captures public HTTP bytes. It does not run a browser, a private
connector, the extension host, embeddings or publication. S3 and S4 are not
required. Live authority and resource controls are currently Linux-only;
Windows and macOS refuse live sync rather than pretend those controls work.
Synthetic port tests run on all three platforms.

## Bind the reviewed local files

Keep bindings separate from the portable manifest. The owner maintains the
reviewed admission evidence; a policy cannot approve itself. Use the strict
`maestro-acquisition-bindings/1` shape:

```json
{
  "schema": "maestro-acquisition-bindings/1",
  "resources": [
    {
      "id": "policy",
      "path": "resources/policy.json",
      "admission": {
        "digest": "<exact SHA-256 of the original reviewed bytes>",
        "platform": "linux",
        "capabilities": [],
        "status": "reviewed",
        "references": []
      }
    }
  ]
}
```

Add one entry for **every** resource in the policy's exact referenced closure:
policy, source decisions, profiles, adapter, address table, qualification,
owner evidence and any transitive members. Preserve the existing Admission
fields and exact digests. The example is a shape, not runnable approval evidence.
An absolute path is used directly; a relative path starts at the bindings
file's directory, not the current working directory. Duplicate, missing,
unknown, changed, unreviewed or unbound resources refuse before fetching.
Grant material never belongs in this file.

## Preview, sync, inspect

1. Preview the seed decisions:

   ```sh
   maestro knowledge acquire preview --manifest "$MANIFEST" \
     --bindings "$BINDINGS" --authority-socket "$AUTHORITY_SOCKET" \
     --authority-uid "$AUTHORITY_UID"
   ```

   Preview makes read-only authority decisions. It makes no DNS, network or
   credential call and starts no frontier work. Public seed URLs may appear in
   this owner-run preview. Missing robots evidence is pending, not an invented
   allow. Preview cannot predict links in a page it has not fetched.

2. Run manual capture with the same inputs:

   ```sh
   maestro --json knowledge acquire sync --manifest "$MANIFEST" \
     --bindings "$BINDINGS" --authority-socket "$AUTHORITY_SOCKET" \
     --authority-uid "$AUTHORITY_UID"
   ```

   Sync uses one admitted HTTP route, one fenced source writer and the kernel
   frontier. Robots, redirects and attachments use the same authority and
   budget controls. Denied destinations are never fetched. OA3's checked-in
   envelope tightens the policy; it never loosens it. Free disk space is measured
   on the actual backing store before and during writes, with a 30 GiB reserve.
   Public capture uses no GPU and does not evict interactive models.

3. Read the recorded result, without fetching again:

   ```sh
   maestro --json knowledge acquire inspect --receipt "$RECEIPT"
   # Or read the latest authorized attempt of a logical run:
   maestro --json knowledge acquire inspect --run "$RUN"
   ```

   Inspect reads durable, currently authorized records only. It opens no
   authority socket, reads no source manifest and invokes no transport. An
   interrupted receipt remains visibly pending. Unknown and denied receipts
   reveal no source metadata.

## Read the result correctly

`completed` counts distinct verified byte captures, not finished source items or publication. `pending` and `discarded` count
references with reasons, not HTTP attempts. A captured page can also have
pending discovery; these are different stages and must not be added together.
Durable output contains digests and fixed reasons, not source URLs or content.
Capture observation times are retained. Receipts freeze the collection digest,
policy/profile closure, scope and caller and never overwrite an earlier run.

Only `complete` is successful completion. Sync and inspect return exit 1 for
partial, blocked, failed or pending work; input refusals return exit 2. Preview
returns exit 0 when its decision report was produced, not when work completed.

- `policy_denial`, `non_fetch_scheme` and `beyond_declared_depth` explain
  discarded references. A declared discovery boundary is scope, not truncation.
- `unresolved_identity`, `run_depth_limit`, `page_budget`, `context_changed`, inventory overflow
  and `discovery_incomplete` remain pending. Overflow is an exact count of
  eligible references not enqueued. An extractor cap does not prove coverage.
- `robots_unavailable` means no cached rules were read. `robots_denied` includes
  disallow, unreadable/stale rules and an unqualified override. Nothing retries
  around those controls. Authority, transport, lease and resource holds are
  also reported without exposing raw errors.

An immediate second sync can reuse acknowledged captures. They are labelled
**captured earlier, not revalidated**, with their original observation time;
there is no second fetch. This is not full or incremental refresh. `--mode`,
repair, withdrawal and schedules are not registered yet. N36 owns revalidation.
Missing media evidence and HTML whose earlier discovery depth is unavailable are explicitly held,
not guessed into a completed crawl.

Every successful invocation releases its source lease after its durable
receipt is written. Errors release owned leases too; a crash falls back to
expiry. If release fails, the diagnostic says when the next sync can start.
The manual MVP admits at most 1,000 declared seeds across sources. The durable
frontier has no item ceiling: 1,000-row cursor pages cover the whole inventory.
The per-run page budget limits new capture attempts, not historical reuse.
Remaining unfinished items are listed with their hold reasons; a complete run
must account for the full frontier, not just its first page.

OS identity authenticates (N05); the kernel principal authorizes; a local
install maps one to `LOCAL`. The composition root freezes that mapping once.
It is not selected by the source, bindings or a CLI flag. Both identities are
retained in the protected inputs; authority checks use the OS identity and
kernel reads use the mapped principal and its current scope grants.

## Verify without a live site

```sh
capped cargo test -p maestro --locked n14_ -- --nocapture
```

These tests use an independently authored public synthetic site and injected
ports. They exercise allowed and denied neighbours, robots, a redirect and an
attachment without real sockets, credentials, S3 or S4. There is no shipped
mock-grant flag or network bypass.
