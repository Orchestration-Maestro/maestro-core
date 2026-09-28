-- The knowledge graph's authority (specs/002-knowledge-graph, FR-S2-002 and
-- FR-S2-003): verified claims, the source locations that support them, and
-- the frozen, ordered claim sets they are admitted in. Every id is the
-- SHA-256 of what it names, so the same content is recorded once. A claim's
-- review state is the only value that ever moves; nothing is replaced or
-- deleted. A time is RFC 3339 UTC with milliseconds.

-- A claim: a subject entity of a kind, named with its exact source spelling,
-- a predicate and a typed literal object, under conditions and version and
-- world validity (unknown, or bounded with open ends as NULL), with the
-- extractor and profile that produced it. `support_count` fixes how many
-- supports it holds, written with it.
CREATE TABLE claims (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  subject_kind TEXT NOT NULL CHECK (subject_kind <> ''),
  subject_name TEXT NOT NULL CHECK (subject_name <> ''),
  predicate TEXT NOT NULL CHECK (predicate IN ('DEFAULTS_TO')),
  object_type TEXT NOT NULL CHECK (object_type IN ('text', 'boolean', 'integer', 'decimal')),
  object_lexeme TEXT NOT NULL,
  conditions_json TEXT NOT NULL CHECK (json_type(conditions_json) = 'object'),
  version_known INTEGER NOT NULL CHECK (version_known IN (0, 1)),
  version_start TEXT,
  version_end TEXT,
  world_known INTEGER NOT NULL CHECK (world_known IN (0, 1)),
  world_start TEXT,
  world_end TEXT,
  extractor TEXT NOT NULL CHECK (extractor <> ''),
  profile_digest TEXT NOT NULL,
  review_state TEXT NOT NULL DEFAULT 'unreviewed'
    CHECK (review_state IN ('unreviewed', 'accepted', 'rejected', 'flagged')),
  support_count INTEGER NOT NULL CHECK (support_count > 0),
  recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (version_known OR (version_start IS NULL AND version_end IS NULL)),
  CHECK (world_known OR (world_start IS NULL AND world_end IS NULL))
) STRICT;

-- A support: the exact bytes of one revision's original Markdown a claim
-- quotes, as a half-open UTF-8 byte span inside a canonical block, with the
-- SHA-256 of those bytes.
CREATE TABLE claim_supports (
  claim_id TEXT NOT NULL REFERENCES claims (id),
  revision_id TEXT NOT NULL REFERENCES revisions (id),
  block_id TEXT NOT NULL CHECK (block_id <> ''),
  span_start INTEGER NOT NULL CHECK (span_start >= 0),
  span_end INTEGER NOT NULL CHECK (span_end > span_start),
  quote_digest TEXT NOT NULL,
  PRIMARY KEY (claim_id, revision_id, block_id, span_start, span_end)
) WITHOUT ROWID, STRICT;

-- A claim set: claims of one collection admitted together, in order.
-- `member_count` fixes how many it holds, written with it.
CREATE TABLE claim_sets (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  member_count INTEGER NOT NULL CHECK (member_count > 0)
) STRICT;

CREATE TABLE claim_set_members (
  claim_set_id TEXT NOT NULL REFERENCES claim_sets (id),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  claim_id TEXT NOT NULL REFERENCES claims (id),
  PRIMARY KEY (claim_set_id, ordinal),
  UNIQUE (claim_set_id, claim_id)
) WITHOUT ROWID, STRICT;

-- A claim is supported only by revisions of its own collection, and a set
-- holds only claims of its own, at ordinals below its count, whoever writes:
-- a set's scope is its collection's, so a row from another collection would
-- show it to a reader of this one.
CREATE TRIGGER claim_supports_quote_their_claims_collection
BEFORE INSERT ON claim_supports
WHEN EXISTS (SELECT 1 FROM claims WHERE id = NEW.claim_id)
  AND EXISTS (SELECT 1 FROM revisions WHERE id = NEW.revision_id)
  AND NOT EXISTS (
    SELECT 1 FROM claims
    JOIN revisions ON revisions.id = NEW.revision_id
    JOIN documents ON documents.id = revisions.document_id
    WHERE claims.id = NEW.claim_id AND documents.collection_id = claims.collection_id)
BEGIN
  SELECT RAISE(ABORT, 'a claim is supported only by revisions of its collection');
END;

CREATE TRIGGER claim_set_members_share_their_sets_collection
BEFORE INSERT ON claim_set_members
WHEN EXISTS (SELECT 1 FROM claim_sets WHERE id = NEW.claim_set_id)
  AND EXISTS (SELECT 1 FROM claims WHERE id = NEW.claim_id)
  AND NOT EXISTS (
    SELECT 1 FROM claim_sets JOIN claims ON claims.id = NEW.claim_id
    WHERE claim_sets.id = NEW.claim_set_id AND claims.collection_id = claim_sets.collection_id
      AND NEW.ordinal < claim_sets.member_count)
BEGIN
  SELECT RAISE(ABORT, 'a claim set holds claims of its collection, at ordinals below its count');
END;

-- A claim holds exactly the supports it was written with, and a set exactly
-- its members: one more is refused, and so is a replacement of one, which
-- inserts while the recorded row still counts.
CREATE TRIGGER claim_supports_are_frozen
BEFORE INSERT ON claim_supports
WHEN (SELECT count(*) FROM claim_supports WHERE claim_id = NEW.claim_id)
  >= (SELECT support_count FROM claims WHERE id = NEW.claim_id)
BEGIN
  SELECT RAISE(ABORT, 'a claim''s supports are frozen');
END;

CREATE TRIGGER claim_set_members_are_frozen
BEFORE INSERT ON claim_set_members
WHEN (SELECT count(*) FROM claim_set_members WHERE claim_set_id = NEW.claim_set_id)
  >= (SELECT member_count FROM claim_sets WHERE id = NEW.claim_set_id)
BEGIN
  SELECT RAISE(ABORT, 'a claim set''s members are frozen');
END;

-- On a connection with recursive triggers, as every kernel connection has,
-- INSERT OR REPLACE fires the delete triggers below on the row it removes.
-- On one without, it removes the row silently, so an insert that would take
-- a recorded claim's or set's id is refused before SQLite resolves the
-- conflict; supports and members are refused by their frozen counts.
CREATE TRIGGER claims_are_never_replaced
BEFORE INSERT ON claims
WHEN EXISTS (SELECT 1 FROM claims WHERE id = NEW.id)
BEGIN
  SELECT RAISE(ABORT, 'a claim is immutable: it is never replaced');
END;

CREATE TRIGGER claims_are_immutable
BEFORE UPDATE OF id, collection_id, subject_kind, subject_name, predicate, object_type,
  object_lexeme, conditions_json, version_known, version_start, version_end, world_known,
  world_start, world_end, extractor, profile_digest, support_count, recorded_at ON claims
BEGIN
  SELECT RAISE(ABORT, 'a claim is immutable: only its review state moves');
END;

CREATE TRIGGER claims_are_never_deleted
BEFORE DELETE ON claims
BEGIN
  SELECT RAISE(ABORT, 'a claim is immutable: it is never deleted');
END;

CREATE TRIGGER claim_supports_never_change
BEFORE UPDATE ON claim_supports
BEGIN
  SELECT RAISE(ABORT, 'a claim support never changes');
END;

CREATE TRIGGER claim_supports_are_never_deleted
BEFORE DELETE ON claim_supports
BEGIN
  SELECT RAISE(ABORT, 'a claim support is never deleted');
END;

CREATE TRIGGER claim_sets_are_never_replaced
BEFORE INSERT ON claim_sets
WHEN EXISTS (SELECT 1 FROM claim_sets WHERE id = NEW.id)
BEGIN
  SELECT RAISE(ABORT, 'a claim set is never replaced');
END;

CREATE TRIGGER claim_sets_never_change
BEFORE UPDATE ON claim_sets
BEGIN
  SELECT RAISE(ABORT, 'a claim set never changes');
END;

CREATE TRIGGER claim_sets_are_never_deleted
BEFORE DELETE ON claim_sets
BEGIN
  SELECT RAISE(ABORT, 'a claim set is never deleted');
END;

CREATE TRIGGER claim_set_members_never_change
BEFORE UPDATE ON claim_set_members
BEGIN
  SELECT RAISE(ABORT, 'a claim set member never changes');
END;

CREATE TRIGGER claim_set_members_are_never_deleted
BEFORE DELETE ON claim_set_members
BEGIN
  SELECT RAISE(ABORT, 'a claim set member is never deleted');
END;
