-- The closed graph vocabulary (specs/002-knowledge-graph, FR-S2-024;
-- architecture 02 §8.2): a claim's subject and entity object are of a listed
-- kind, and its predicate a listed relation type but `ALIAS_OF`, which is a
-- reviewed identity record, never a claim. `DEFAULTS_TO` alone has a typed
-- literal object; every other predicate has an entity object, named by kind
-- and exact source spelling in the claim's own collection. The kind and
-- predicate lists are `EntityKind` and `Predicate` in
-- `src/vocabulary.rs`; the migration runner expands the placeholders below
-- from those definitions, so SQLite and Rust enforce the same vocabulary.
--
-- The tables of 0012 are rebuilt with every row, rowid, id, digest, review
-- state and time unchanged: a row 0012 admitted is a valid literal claim of
-- the new form, but for a subject kind outside the list, which the kernel
-- refuses before this runs, whole and with nothing changed (see
-- `src/store/migration.rs`). With foreign keys enforced, renaming `claims`
-- points its supports and members at the old table, so they are rebuilt
-- against the new one before the old is dropped; every trigger is made again
-- once the rows are in place.

ALTER TABLE claims RENAME TO claims_0012;

CREATE TABLE claims (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  subject_kind TEXT NOT NULL CHECK (subject_kind IN ({entity_kinds})),
  subject_name TEXT NOT NULL CHECK (subject_name <> ''),
  predicate TEXT NOT NULL CHECK (predicate IN ({claim_predicates})),
  object_type TEXT CHECK (object_type IN ('text', 'boolean', 'integer', 'decimal')),
  object_lexeme TEXT,
  object_kind TEXT CHECK (object_kind IN ({entity_kinds})),
  object_name TEXT CHECK (object_name <> ''),
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
  CHECK (CASE WHEN predicate = 'DEFAULTS_TO'
    THEN object_type IS NOT NULL AND object_lexeme IS NOT NULL
      AND object_kind IS NULL AND object_name IS NULL
    ELSE object_type IS NULL AND object_lexeme IS NULL
      AND object_kind IS NOT NULL AND object_name IS NOT NULL END),
  CHECK (version_known OR (version_start IS NULL AND version_end IS NULL)),
  CHECK (world_known OR (world_start IS NULL AND world_end IS NULL))
) STRICT;

INSERT INTO claims (rowid, id, collection_id, subject_kind, subject_name, predicate,
  object_type, object_lexeme, conditions_json, version_known, version_start, version_end,
  world_known, world_start, world_end, extractor, profile_digest, review_state,
  support_count, recorded_at)
SELECT rowid, id, collection_id, subject_kind, subject_name, predicate, object_type,
  object_lexeme, conditions_json, version_known, version_start, version_end, world_known,
  world_start, world_end, extractor, profile_digest, review_state, support_count,
  recorded_at
FROM claims_0012 ORDER BY rowid;

CREATE TABLE claim_supports_0013 (
  claim_id TEXT NOT NULL REFERENCES claims (id),
  revision_id TEXT NOT NULL REFERENCES revisions (id),
  block_id TEXT NOT NULL CHECK (block_id <> ''),
  span_start INTEGER NOT NULL CHECK (span_start >= 0),
  span_end INTEGER NOT NULL CHECK (span_end > span_start),
  quote_digest TEXT NOT NULL,
  PRIMARY KEY (claim_id, revision_id, block_id, span_start, span_end)
) WITHOUT ROWID, STRICT;

INSERT INTO claim_supports_0013 SELECT * FROM claim_supports;
DROP TABLE claim_supports;
ALTER TABLE claim_supports_0013 RENAME TO claim_supports;

CREATE TABLE claim_set_members_0013 (
  claim_set_id TEXT NOT NULL REFERENCES claim_sets (id),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  claim_id TEXT NOT NULL REFERENCES claims (id),
  PRIMARY KEY (claim_set_id, ordinal),
  UNIQUE (claim_set_id, claim_id)
) WITHOUT ROWID, STRICT;

INSERT INTO claim_set_members_0013 SELECT * FROM claim_set_members;
DROP TABLE claim_set_members;
ALTER TABLE claim_set_members_0013 RENAME TO claim_set_members;

DROP TABLE claims_0012;

-- 0012's triggers, made again on the rebuilt tables; a claim's object
-- columns are frozen with the rest.
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

CREATE TRIGGER claims_are_never_replaced
BEFORE INSERT ON claims
WHEN EXISTS (SELECT 1 FROM claims WHERE id = NEW.id)
BEGIN
  SELECT RAISE(ABORT, 'a claim is immutable: it is never replaced');
END;

CREATE TRIGGER claims_are_immutable
BEFORE UPDATE OF id, collection_id, subject_kind, subject_name, predicate, object_type,
  object_lexeme, object_kind, object_name, conditions_json, version_known, version_start,
  version_end, world_known, world_start, world_end, extractor, profile_digest, support_count,
  recorded_at ON claims
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
