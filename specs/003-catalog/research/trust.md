# C09 public attestation probe and trust measurements

**Measurement:** public-only; no GitHub credential used. The artifact and
attestation bundle are real `cli/cli` release evidence. The same pinned bytes
and signer parameters are used by the passing and wrong-signer controls. Both
fixture records live in `tests/fixtures/catalog/trust/`.

## Artifact and verifier candidate

- Public artifact: `cli/cli` release `v2.98.0`,
  `gh_2.98.0_linux_amd64.tar.gz`, 14,863,663 bytes,
  SHA-256 `3b8ac6b30336802fc1a858d7c084e11cdf24ac1a761ca90b68022d7d729208de`.
- Verifier candidate: `gh version 2.98.0 (2026-08-20)`, executable SHA-256
  `62885b97de6a0cd85e616cdd94bcda908bf5cf1018094385892b05cea3537163`.
  This is measured evidence, not the OA4 executable pin; OA4 must still supply
  the checksum-verified standalone binary and authority binding.
- A real public REST request to
  `/repos/cli/cli/attestations/sha256:3b8ac6b30336802fc1a858d7c084e11cdf24ac1a761ca90b68022d7d729208de`
  returned HTTP 200 and two attestation bundle URLs without authentication.
  The response's `X-RateLimit-Limit` was 60 and remaining quota 55.
  Download both returned `bundle_url` values from their signed blob URLs (not
  from `api.github.com`), decompress their `application/x-snappy` payloads to
  JSON bundle objects, and join them as newline-delimited JSON for `--bundle`.
  The measured JSONL bytes SHA-256 is
  `afa4c46c4a2bc05e0e905b7299c426ac03971fc767d06dcd2a6d2d0d282796ca`.
  Signed blob URLs expire; re-fetch the public list instead of retaining a
  SAS URL.

## Frozen verifier contracts and outcomes

Online verification looks up the attestation against the named repository;
offline verification uses the locally fetched Sigstore bundle JSONL. Each
refusal is a non-zero exit and has no executor effect. The `--bundle` probe
runs the actual verifier against cryptographically signed public attestations,
not a fabricated verifier response.

Valid expected identity and the online form:

```sh
gh attestation verify <artifact> \
  -R cli/cli \
  --signer-workflow cli/cli/.github/workflows/deployment.yml \
  --format json
```

Offline valid form:

```sh
gh attestation verify <artifact> \
  -R cli/cli \
  --bundle bundles.jsonl \
  --signer-workflow cli/cli/.github/workflows/deployment.yml \
  --format json
```

The online valid command with empty `GH_CONFIG_DIR` and `GH_TOKEN` /
`GITHUB_TOKEN` unset exits 4 before API lookup, with the CLI's auth-required
message. The offline valid command exits 0 and outputs one verified result:
issuer `https://token.actions.githubusercontent.com`; signer
`https://github.com/cli/cli/.github/workflows/deployment.yml@refs/heads/trunk`;
the attested subject includes the exact local artifact SHA-256 above. Its
19,148-byte JSON stdout SHA-256 is
`2fdbdbde6443526d5a4e627fc23df864cf7bac7459ec42793d37129d6a6d4d44`.

Wrong identity changes only `--signer-workflow`, keeping the same artifact,
repository lookup and bundle:

```sh
gh attestation verify <artifact> \
  -R cli/cli \
  --signer-workflow github/docs/.github/workflows/releases.yml \
  --format json

gh attestation verify <artifact> \
  -R cli/cli \
  --bundle bundles.jsonl \
  --signer-workflow github/docs/.github/workflows/releases.yml \
  --format json
```

Online with no credential exits 4 before lookup (same auth-required stderr as
valid; SHA-256 `69f5519080eaf4c3a8900ff6c94fe8295d61829c17161fb23806005b55bc84b9`).
Offline exits 1 after loading the real bundles; stdout is empty (SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`) and
stderr is exactly `Error: verifying with issuer "sigstore.dev"\n` (45 bytes,
SHA-256 `486941643d59bcaec905305a9ad2843539956d60f735745f77a05db562cfda19`).
This distinguishes identity refusal from a missing-attestation lookup. Full
exit/output digests and argv are recorded in each fixture JSON.

## Online credential and refresh budget

Unauthenticated public REST primary rate limit is 60 requests/hour; the
standard authenticated REST limit is 5,000 requests/hour per user (GitHub REST
rate-limit documentation). The measured bundle-list step costs one REST request
and returned two signed blob URLs; blob downloads use their blob host and are
not additional `api.github.com` REST calls. Do not treat this offline fetch as
an online `gh attestation verify` measurement: `gh` stopped before the online
lookup without credentials. OA4 must provide the least-scope binding and C09
must measure the online lookup's request count with that approved credential.

At the required five-minute refresh, the budget is
`12 × online API requests per refresh × active laptops per trust root` per
hour. The lookup count remains pending OA4; the request must be compared with
60/hour unauthenticated and 5,000/hour authenticated limits, plus concurrent
roots and retry/backoff. Proposed credential for OA4 confirmation: fine-grained
token restricted to public repositories, read-only, no permissions. This is a
proposal only; a zero-permission token has not been tested and OA4 must confirm
the least scope that permits the online lookup. A broad-scope local login was
not used.
