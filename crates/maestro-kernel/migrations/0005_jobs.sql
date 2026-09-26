-- Jobs (plan D5, building block B4): long work, such as an import, a
-- preparation or a publication, run under a lease and resumed from its
-- progress. The journal holds each change of a job and each of its steps on
-- the job's stream, `job/<id>`, recorded in the write that makes it.
--
-- A job's ID is a ULID. Its idempotency key is the SHA-256 digest, in hex, of
-- its kind, its scope and the frozen inputs its caller chose, and its attempt
-- counts the jobs of that key, 1 for the first. At most one job of a key is
-- queued, running or succeeded: the one a retried command finds. A job that
-- failed or was cancelled leaves its key to the next attempt. A job may hold a
-- resource, such as the publication of a collection, and at most one job
-- queued or running holds each.
--
-- A job is queued, then running, then succeeded, failed or cancelled, or
-- cancelled while queued; once it ended, it never changes, and no job is ever
-- replaced nor deleted. It holds a lease while it runs, and only then: its
-- holder, its number (1 for the first lease, one more for each takeover, and
-- never fewer), its last heartbeat and its expiry, RFC 3339 in UTC to the
-- millisecond, from the caller's clock. Its outcome, JSON text, is recorded
-- when it ends.
CREATE TABLE jobs (
  id TEXT PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  attempt INTEGER NOT NULL CHECK (attempt >= 1),
  scope TEXT NOT NULL,
  resource TEXT,
  state TEXT NOT NULL
    CHECK (state IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
  lease_number INTEGER NOT NULL DEFAULT 0 CHECK (lease_number >= 0),
  lease_holder TEXT,
  lease_heartbeat TEXT CHECK (lease_heartbeat GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9]Z'),
  lease_expires TEXT CHECK (lease_expires GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9]Z'),
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

-- One job at most, queued or running, holds each resource.
CREATE UNIQUE INDEX jobs_one_per_resource ON jobs (resource)
WHERE resource IS NOT NULL AND state IN ('queued', 'running');

-- A job is never replaced nor deleted, whoever writes: its row stays, as its
-- stream in the journal does; a retention task that removes old jobs will
-- lift this by a migration of its own. INSERT OR REPLACE removes the rows it
-- conflicts with without firing a delete trigger, and an upsert updates the
-- row it conflicts with, so an insert that would take the ID or the place of a
-- job (its attempt of its key, its live key, its resource) is refused before
-- SQLite resolves the conflict.
CREATE TRIGGER jobs_are_never_replaced
BEFORE INSERT ON jobs
WHEN EXISTS (SELECT 1 FROM jobs WHERE id = NEW.id)
  OR EXISTS (SELECT 1 FROM jobs
    WHERE idempotency_key = NEW.idempotency_key AND attempt = NEW.attempt)
  OR (NEW.state IN ('queued', 'running', 'succeeded') AND EXISTS (SELECT 1 FROM jobs
    WHERE idempotency_key = NEW.idempotency_key AND state IN ('queued', 'running', 'succeeded')))
  OR (NEW.resource IS NOT NULL AND NEW.state IN ('queued', 'running') AND EXISTS (SELECT 1 FROM jobs
    WHERE resource = NEW.resource AND state IN ('queued', 'running')))
BEGIN
  SELECT RAISE(ABORT, 'a job is never replaced: no insert takes the ID or the place of a job');
END;

CREATE TRIGGER jobs_are_never_deleted
BEFORE DELETE ON jobs
BEGIN
  SELECT RAISE(ABORT, 'a job is never deleted: its row stays, as its stream does');
END;

-- A job that ended never changes, whoever writes: its outcome is final.
CREATE TRIGGER jobs_that_ended_never_change
BEFORE UPDATE ON jobs
WHEN OLD.state IN ('succeeded', 'failed', 'cancelled')
BEGIN
  SELECT RAISE(ABORT, 'a job that ended never changes: its outcome is final');
END;

-- A job moves only forward, whoever writes: from queued to running or
-- cancelled, and from running to succeeded, failed or cancelled; a takeover
-- keeps it running. The trigger above holds a job that ended.
CREATE TRIGGER jobs_move_only_forward
BEFORE UPDATE OF state ON jobs
WHEN OLD.state <> NEW.state
  AND OLD.state IN ('queued', 'running')
  AND NOT (OLD.state = 'queued' AND NEW.state IN ('running', 'cancelled'))
  AND NOT (OLD.state = 'running' AND NEW.state IN ('succeeded', 'failed', 'cancelled'))
BEGIN
  SELECT RAISE(ABORT,
    'a job moves only from queued to running or cancelled, and from running to an outcome');
END;

-- A lease number never decreases, whoever writes, so a lease taken over never
-- becomes the job's again.
CREATE TRIGGER jobs_lease_numbers_never_decrease
BEFORE UPDATE OF lease_number ON jobs
WHEN NEW.lease_number < OLD.lease_number
BEGIN
  SELECT RAISE(ABORT,
    'a lease number never decreases: a lease taken over never becomes the job''s again');
END;
