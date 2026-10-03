# Acquisition authority

Acquisition reads decisions through `Authority`; it cannot write grants. Source
manifests, model output, review evidence and `--yes` never authenticate an owner.
Each dispatch rechecks the current grant, exact principal, operation, collection
scope, source, account, resource and expiry. Revocation closes future dispatch.

## Qualified hosts

Only the Linux Unix-socket adapter is implemented. Windows and macOS refuse with
`authority unqualified`; cross-compilation does not qualify those platforms.
Real source/account grants remain blocked on exact OA4a receipts, and robots
exceptions on OA4b receipts. Persistent host setup needs separate authorization.
Do not run the following setup against a real host before that authorization.

1. Provision three distinct non-root identities: owner/authority, pipeline and
   connector. Acquisition and connectors must actually run under their separate
   unprivileged identities, with no ability to assume the owner identity.
2. Provision an owner-owned `0700` store and an owner-controlled socket directory
   traversable by the clients. No symlinks or group/other-writable ancestors are
   allowed, except a root-owned sticky temporary directory. Put the same build of
   `maestro` beside the authorized launcher, with no group/other write permission.
3. Supply a host configuration outside source manifests. Its strict fields are
   `store` (absolute directory), `socket` (absolute socket path), `owner_uid`,
   `pipeline_uid`, `connector_uid` and `launcher` (absolute executable path).
   The launcher is an admin-provisioned trust boundary: it receives the UID,
   binary and probe arguments, drops to that exact UID with cleared groups and
   no retained privilege, and returns the child's status unchanged. Launcher
   stdout is diagnostic only, never qualification evidence.
   Serving never invokes the launcher, `sudo` or any child process.
4. Run `maestro authority qualify --config "$AUTHORITY_CONFIG"` as the owner.
   This launches the real pipeline and connector create/edit/delete probes and
   requires each probe to connect to a one-shot qualification socket. Linux
   peer credentials must match the expected UID, and a strict report must show
   all three mutations denied. A second connection or existing endpoint refuses.
   The qualifier also checks the canary digest, owner and mode remain unchanged.
   Connection and launcher-exit waits use one cumulative deadline per UID: default
   15 seconds, configurable with `--probe-timeout-seconds` (1–300). A protected qualification receipt
   binds the identities, paths, store owner/modes/device/inode, launcher identity
   and exact probe binary. Failed requalification probes remove the old receipt.
5. Run `maestro authority serve --config "$AUTHORITY_CONFIG"` as the owner.
   Missing/stale qualification or broken separation refuses before opening a
   grant writer. Requalify after any bound host change. Restart the service after
   changes to host configuration or identities; do not change host bindings while
   it is serving. A socket left by a killed service requires owner inspection
   and removal before restart; startup does not unlink an existing endpoint.

The authority directory contains `authority.sqlite3` and its private audit.
Grant create/edit/revoke and the audit entry are one durable SQLite transaction.
An owner command refused for an already-expired grant also records an audit entry.
Read-only expiry decisions return `Expired` with the matched grant ID and leave
store bytes unchanged; acquisition receipts are N06's responsibility.
The socket is accessible to clients, but the kernel authenticates both ends with
peer credentials. Only the configured owner UID can mutate; an unprivileged
principal can read only its own decisions. The pipeline's `AuthoritySocket`
exposes no mutation operation and refuses a same-user authority fallback.

## Exact owner command

An owner confirms the complete grant twice in a protected local request file.
The owner CLI opens without following symlinks, checks on that descriptor that
it is a regular file owned by the authority UID without group/other write bits,
and reads from that same descriptor. This applies to owner inspection too:

```json
{
  "action": "grant",
  "grant": {
    "id": "synthetic-grant",
    "principal": "65534",
    "operation": "fetch",
    "target": {
      "scope": "workspace/default/collection/synthetic",
      "source": "handbook",
      "account": "public",
      "resource": "https://synthetic.example/manual"
    },
    "expires_at": "2099-01-01T00:00:00Z"
  },
  "confirmation": {
    "id": "synthetic-grant",
    "principal": "65534",
    "operation": "fetch",
    "target": {
      "scope": "workspace/default/collection/synthetic",
      "source": "handbook",
      "account": "public",
      "resource": "https://synthetic.example/manual"
    },
    "expires_at": "2099-01-01T00:00:00Z"
  }
}
```

This is synthetic data, not a live authorization. Expiry is UTC RFC3339 with
whole-second precision. N03's canonical URL spelling is used for matching,
including dropping an explicit default `:443` port. IP hosts and non-UTC expiry
offsets refuse. Resource URLs are exact HTTPS targets without userinfo,
query strings or fragments. An account role is required even for public reads.
`robots_override` is a separate operation, never implied by `fetch`.

Run the authenticated command as the owner:

```sh
maestro authority request --socket "$AUTHORITY_SOCKET" \
  --authority-uid "$AUTHORITY_UID" --file "$EXACT_GRANT_REQUEST"
```

The same command edits a grant using its ID and a newly confirmed full record.
To revoke, use `"action": "revoke"` with the exact stored grant and confirmation.
Mismatched scope/target/expiry, generic confirmation and forged approval fields
refuse. The file supplies the intent, not identity: even an identical request
from the pipeline or connector UID cannot mutate anything.

A client's read-only request uses `action`, `principal`, `operation` and `target`:
`{"action":"decide","principal":"65534","operation":"fetch","target":...}`.
The owner CLI can inspect these read-only decisions through its authenticated
owner path; the pipeline adapter still refuses a same-UID authority. Non-owner
decision files remain untrusted intent: the service authenticates their principal
through kernel credentials and refuses all mutations.
The authority uses its own clock, not the request's time. Refusals carry no grant
or source details. JSON frames are capped at 64 KiB with a cumulative two-second
read deadline; local command/configuration files are capped at 16 KiB.

## Synthetic qualification evidence

The default suite never invokes `sudo`. The ignored `n05_needs_sudo` fixtures use
only temporary directories, the invoking owner UID, pipeline UID 65534 and
connector UID 65533. The launcher alone uses `sudo -n setpriv` to drop privilege;
no users, services or persistent host grants are created. Run these only with
explicit host-test authorization:

```sh
capped cargo nextest run -p maestro --locked n05_needs_sudo --run-ignored only --test-threads 1
```

A passing synthetic fixture is not approval of real grants or host deployment.
