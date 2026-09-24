# Bundles need freshness and revocation, not only signatures

Status: accepted, 2026-09-24.

A signature proves a catalog bundle is authentic; it does not prove the bundle
is still permitted. Following The Update Framework's threat model, the runtime
checks four things before governed use: the attestation (who built it), an
expiring timestamp record (freshness), a small revocation list fetched
frequently (entries and whole bundles withdrawn between releases) and a
monotonic version floor (no rollback to a revoked or older security version).
Offline, the last verified records apply until their expiry, and the remaining
window is shown; after expiry, governed runs refuse to start.

## Considered options

- Signatures and checksums only: an attacker or a stale mirror can replay an old
  vulnerable bundle; a withdrawn agent keeps running.
- Full TUF implementation (roles, delegations, threshold keys): heavier than one
  publisher needs today; the record formats stay compatible with adopting it
  later.

## Consequences

The manifests release workflow also publishes the timestamp and revocation
records; key rotation and emergency revocation have written procedures and
tests; rollback and resume can never restore a revoked version.
