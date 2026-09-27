# Private-content refusal

`maestro-conventions`' existing public policies check repository structure,
paths and settings. The public Rust tests exercise the content matcher only with
invented text. A passing public test or secret scan is **not** a comparison
against the private collection.

Before pushing to a public repository, run the trusted `privacy/push.sh`
preflight from the private collection repository. It requires an external,
current bank and key, resolves every explicit `source:destination` refspec to
object IDs, scans objects reachable from those refs but from no fetched branch
on the target remote, then pushes those frozen source IDs. Use full
`refs/heads/...` or `refs/tags/...` refnames; wildcard and deletion refspecs are
refused. The scanner reads Git objects with plumbing commands and never checks
out candidate content. A branch based on an existing remote branch scans its
new commits and blobs; a genuinely empty remote requires its full reachable
history to be scanned.

The private collector reads the private repository inputs and the documents
listed in the clean-corpus manifest. It writes a SQLite bank, key, exact-unit
allowlist and opaque inventory identifier only beneath
`~/.local/share/maestro-privacy/`; private files have mode `600`, and the
directory has mode `700`. They are not repository files. Each refresh
fingerprints the current input snapshot and allowlist. Each bank build records
an external HMAC-SHA-256 digest of the complete SQLite file. Every push streams
the bank file through that HMAC before opening it; full SQLite and logical-row
validation runs during builds and through `maestro-privacy verify-bank`, not on
every push. Private mode refuses a missing, changed, wrong-key or stale bank.

Normalization v1 applies Unicode NFKC and lowercase, retains alphanumeric
words, and collapses punctuation/whitespace. The bank stores only keyed
SHA-256 tags for exact file bytes, overlapping eight-token shingles from
collector-selected prose, and complete four-to-seven-token question units.
The private selectors exclude identifiers and other metadata; candidate JSON
string values are decoded before scanning. A matching file or shingle refuses
the push. A short-unit match requires an exact normalized candidate window and
is suppressed only by its exact private allowlist entry. Product vocabulary
alone is not a match. Candidate reports contain object IDs, unit ordinals,
line/byte locations and counts only—never matched text, paths, excerpts or
fingerprints. The scanner exits `0` for clean, `1` for a finding and `2` when
evidence is incomplete or invalid; every nonzero result blocks the wrapper.

This is accidental-copy detection, not a semantic classifier. Short text
outside selected question units, paraphrases, translations and obfuscation are
not reliably covered. Public checks use synthetic inputs only; a private
comparison occurs only when the owner preflight has a current external bank.
