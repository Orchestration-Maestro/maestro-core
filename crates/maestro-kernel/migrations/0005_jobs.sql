-- Jobs (plan D5, building block B4): long work, such as an import, a
-- preparation or a publication, run under a lease and resumed from its
-- progress, which the journal holds on the job's stream, `job/<id>`.
--
-- A job's ID is a ULID. Its idempotency key is the SHA-256 digest, in hex, of
-- its kind and of the frozen inputs its caller chose, and its attempt counts
-- the jobs of that key, 1 for the first. At most one job of a key is queued,
-- running or succeeded: the one a retried command finds. A job that failed or
-- was cancelled leaves its key to the next attempt.
--
-- A job is queued, then running, then succeeded, failed or cancelled, or
-- cancelled while queued. It holds a lease while it runs, and only then: its
-- holder, its number (1 for the first lease, one more for each takeover), its
-- last heartbeat and its expiry, RFC 3339 in UTC with milliseconds, from the
-- caller's clock. Its outcome, JSON text, is recorded when it ends.
CREATE TABLE jobs (
  id TEXT PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  attempt INTEGER NOT NULL CHECK (attempt >= 1),
  scope TEXT NOT NULL,
  state TEXT NOT NULL
    CHECK (state IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
  lease_number INTEGER NOT NULL DEFAULT 0 CHECK (lease_number >= 0),
  lease_holder TEXT,
  lease_heartbeat TEXT,
  lease_expires TEXT,
  outcome_json TEXT CHECK (json_valid(outcome_json)),
  UNIQUE (idempotency_key, attempt),
  CHECK ((state = 'running') = (lease_holder IS NOT NULL)),
  CHECK ((lease_holder IS NULL) = (lease_heartbeat IS NULL)),
  CHECK ((lease_holder IS NULL) = (lease_expires IS NULL)),
  CHECK (state <> 'running' OR lease_number >= 1),
  CHECK ((state IN ('succeeded', 'failed', 'cancelled')) = (outcome_json IS NOT NULL))
) STRICT;

-- One job of a key at a time is queued, running or succeeded, so one lease at
-- most is live on a key.
CREATE UNIQUE INDEX jobs_live_per_key ON jobs (idempotency_key)
WHERE state IN ('queued', 'running', 'succeeded');
