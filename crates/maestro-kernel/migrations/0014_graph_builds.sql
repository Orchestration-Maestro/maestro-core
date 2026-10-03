-- Graph builds (specs/002-knowledge-graph, FR-S2-005): a collection's claims
-- extracted under a kernel job's lease, one source revision per batch, each
-- batch's receipt written with the job's progress in one write; and the
-- once-only attachment of a finished build's frozen claim set to a
-- generation. Nothing here is replaced, changed or deleted, but a build's
-- claim set, set once when it finishes.

-- A build: the job it runs under and its frozen inputs, the collection, the
-- extractor and the digest of its profile, the ordered source revisions, one
-- batch each, and its budgets: at most `max_claims` claims, and at most
-- `max_rejections` rejections kept with their reason. Its claim set is
-- recorded once every batch is, and never before: a partial build has none.
CREATE TABLE graph_builds (
  job_id TEXT PRIMARY KEY NOT NULL REFERENCES jobs (id),
  collection_id TEXT NOT NULL REFERENCES collections (id),
  extractor TEXT NOT NULL CHECK (extractor <> ''),
  profile_digest TEXT NOT NULL,
  sources_json TEXT NOT NULL CHECK (json_type(sources_json) = 'array'),
  batch_count INTEGER NOT NULL CHECK (batch_count > 0),
  max_claims INTEGER NOT NULL CHECK (max_claims > 0),
  max_rejections INTEGER NOT NULL CHECK (max_rejections >= 0),
  claim_set_id TEXT REFERENCES claim_sets (id),
  CHECK (json_array_length(sources_json) = batch_count)
) STRICT;

-- A batch's receipt: the source revision it extracted, the lease that wrote
-- it, the digest of its content, how many claims it accepted, and how many
-- rejections it made and kept. Batches are written in order.
CREATE TABLE graph_build_batches (
  job_id TEXT NOT NULL REFERENCES graph_builds (job_id),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  revision_id TEXT NOT NULL REFERENCES revisions (id),
  lease_number INTEGER NOT NULL CHECK (lease_number >= 1),
  content_digest TEXT NOT NULL,
  claim_count INTEGER NOT NULL CHECK (claim_count >= 0),
  rejected INTEGER NOT NULL CHECK (rejected >= 0),
  kept INTEGER NOT NULL CHECK (kept >= 0 AND kept <= rejected),
  PRIMARY KEY (job_id, ordinal)
) WITHOUT ROWID, STRICT;

-- The claims a batch accepted, in its order.
CREATE TABLE graph_build_claims (
  job_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  claim_id TEXT NOT NULL REFERENCES claims (id),
  PRIMARY KEY (job_id, ordinal, position),
  FOREIGN KEY (job_id, ordinal) REFERENCES graph_build_batches (job_id, ordinal)
) WITHOUT ROWID, STRICT;

-- The rejections a build kept, in its order across batches, with the batch
-- that made each: at most the build's `max_rejections`.
CREATE TABLE graph_build_rejections (
  job_id TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  ordinal INTEGER NOT NULL,
  revision_id TEXT NOT NULL,
  block_id TEXT,
  reason TEXT NOT NULL CHECK (reason <> ''),
  PRIMARY KEY (job_id, position),
  FOREIGN KEY (job_id, ordinal) REFERENCES graph_build_batches (job_id, ordinal)
) WITHOUT ROWID, STRICT;

-- A generation's graph: the claim set of one finished build of its
-- collection, attached once, while the generation is not yet published.
CREATE TABLE graph_attachments (
  generation_id INTEGER PRIMARY KEY NOT NULL REFERENCES generations (id),
  job_id TEXT NOT NULL REFERENCES graph_builds (job_id),
  claim_set_id TEXT NOT NULL REFERENCES claim_sets (id),
  attached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- A batch follows the last one of its build, for the source at its ordinal,
-- and only before the build has its claim set.
CREATE TRIGGER graph_build_batches_follow_in_order
BEFORE INSERT ON graph_build_batches
WHEN NOT EXISTS (
  SELECT 1 FROM graph_builds
  WHERE job_id = NEW.job_id AND claim_set_id IS NULL
    AND json_extract(sources_json, '$[' || NEW.ordinal || ']') = NEW.revision_id
    AND NEW.ordinal = (SELECT count(*) FROM graph_build_batches WHERE job_id = NEW.job_id))
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s batches follow its sources in order, before it ends');
END;

-- A build keeps at most its budget of rejections.
CREATE TRIGGER graph_build_rejections_are_bounded
BEFORE INSERT ON graph_build_rejections
WHEN (SELECT count(*) FROM graph_build_rejections WHERE job_id = NEW.job_id)
  >= (SELECT max_rejections FROM graph_builds WHERE job_id = NEW.job_id)
BEGIN
  SELECT RAISE(ABORT, 'a graph build keeps no more rejections than its budget');
END;

-- A build's claim set is set once, when every batch is recorded, to a set of
-- its collection; nothing else of a build ever moves.
CREATE TRIGGER graph_builds_end_once
BEFORE UPDATE ON graph_builds
WHEN OLD.claim_set_id IS NOT NULL
  OR NEW.claim_set_id IS NULL
  OR NEW.job_id IS NOT OLD.job_id OR NEW.collection_id IS NOT OLD.collection_id
  OR NEW.extractor IS NOT OLD.extractor OR NEW.profile_digest IS NOT OLD.profile_digest
  OR NEW.sources_json IS NOT OLD.sources_json OR NEW.batch_count IS NOT OLD.batch_count
  OR NEW.max_claims IS NOT OLD.max_claims OR NEW.max_rejections IS NOT OLD.max_rejections
  OR (SELECT count(*) FROM graph_build_batches WHERE job_id = OLD.job_id) <> OLD.batch_count
  OR NOT EXISTS (SELECT 1 FROM claim_sets
    WHERE id = NEW.claim_set_id AND collection_id = OLD.collection_id)
BEGIN
  SELECT RAISE(ABORT, 'a graph build ends once, with every batch, in a set of its collection');
END;

-- A generation gets the claim set of a finished build of its collection,
-- while it is building or verified: a published generation's pins never see
-- new claims.
CREATE TRIGGER graph_attachments_bind_unpublished_generations
BEFORE INSERT ON graph_attachments
WHEN NOT EXISTS (
  SELECT 1 FROM graph_builds JOIN generations ON generations.id = NEW.generation_id
  WHERE graph_builds.job_id = NEW.job_id AND graph_builds.claim_set_id = NEW.claim_set_id
    AND generations.collection_id = graph_builds.collection_id
    AND generations.state IN ('building', 'verified'))
BEGIN
  SELECT RAISE(ABORT,
    'a generation gets the claim set of a finished build of its collection, before publication');
END;

-- On a connection with recursive triggers, INSERT OR REPLACE fires the
-- delete triggers below; on one without, an insert that would take a
-- recorded row's key is refused before SQLite resolves the conflict.
CREATE TRIGGER graph_builds_are_never_replaced
BEFORE INSERT ON graph_builds
WHEN EXISTS (SELECT 1 FROM graph_builds WHERE job_id = NEW.job_id)
BEGIN
  SELECT RAISE(ABORT, 'a graph build is never replaced');
END;

CREATE TRIGGER graph_builds_are_never_deleted
BEFORE DELETE ON graph_builds
BEGIN
  SELECT RAISE(ABORT, 'a graph build is never deleted');
END;

CREATE TRIGGER graph_build_batches_never_change
BEFORE UPDATE ON graph_build_batches
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s batch never changes');
END;

CREATE TRIGGER graph_build_batches_are_never_deleted
BEFORE DELETE ON graph_build_batches
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s batch is never deleted');
END;

CREATE TRIGGER graph_build_claims_are_frozen
BEFORE INSERT ON graph_build_claims
WHEN (SELECT count(*) FROM graph_build_claims
    WHERE job_id = NEW.job_id AND ordinal = NEW.ordinal)
  >= (SELECT claim_count FROM graph_build_batches
    WHERE job_id = NEW.job_id AND ordinal = NEW.ordinal)
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s batch holds the claims it was written with');
END;

CREATE TRIGGER graph_build_claims_never_change
BEFORE UPDATE ON graph_build_claims
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s claim never changes');
END;

CREATE TRIGGER graph_build_claims_are_never_deleted
BEFORE DELETE ON graph_build_claims
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s claim is never deleted');
END;

CREATE TRIGGER graph_build_rejections_are_frozen
BEFORE INSERT ON graph_build_rejections
WHEN (SELECT count(*) FROM graph_build_rejections
    WHERE job_id = NEW.job_id AND ordinal = NEW.ordinal)
  >= (SELECT kept FROM graph_build_batches
    WHERE job_id = NEW.job_id AND ordinal = NEW.ordinal)
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s batch keeps the rejections it was written with');
END;

CREATE TRIGGER graph_build_rejections_never_change
BEFORE UPDATE ON graph_build_rejections
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s rejection never changes');
END;

CREATE TRIGGER graph_build_rejections_are_never_deleted
BEFORE DELETE ON graph_build_rejections
BEGIN
  SELECT RAISE(ABORT, 'a graph build''s rejection is never deleted');
END;

CREATE TRIGGER graph_attachments_are_never_replaced
BEFORE INSERT ON graph_attachments
WHEN EXISTS (SELECT 1 FROM graph_attachments WHERE generation_id = NEW.generation_id)
BEGIN
  SELECT RAISE(ABORT, 'a generation''s graph is attached once');
END;

CREATE TRIGGER graph_attachments_never_change
BEFORE UPDATE ON graph_attachments
BEGIN
  SELECT RAISE(ABORT, 'a generation''s graph attachment never changes');
END;

CREATE TRIGGER graph_attachments_are_never_deleted
BEFORE DELETE ON graph_attachments
BEGIN
  SELECT RAISE(ABORT, 'a generation''s graph attachment is never deleted');
END;
