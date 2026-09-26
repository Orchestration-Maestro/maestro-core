-- The pipeline's records (docs/architecture/01 §11; plan D6 and D9): the
-- collections and the sources they declare, the documents each source holds
-- and their revisions, the quality dispositions, occurrences and
-- near-duplicate groups of those revisions, the chunk sets cut from them, and
-- the search generations built from a chunk set. Every table a later task of
-- S1 writes is here, so none needs a migration of its own; the jobs, the
-- events, and S6's frontier items and captures have theirs. A JSON column
-- holds the JSON its name says; a time is RFC 3339 UTC with milliseconds.

-- A collection, as its declaration names it (01 §1); `profiles_json` maps
-- each stage to its profile.
CREATE TABLE collections (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL,
  visibility TEXT NOT NULL,
  profiles_json TEXT NOT NULL CHECK (json_type(profiles_json) = 'object')
) STRICT;

-- A source is declared inside its collection, so its id is unique there only.
-- An import has no transport.
CREATE TABLE sources (
  collection_id TEXT NOT NULL REFERENCES collections (id),
  id TEXT NOT NULL,
  kind TEXT NOT NULL,
  transport TEXT,
  reference TEXT NOT NULL,
  profiles_json TEXT NOT NULL CHECK (json_type(profiles_json) = 'object'),
  PRIMARY KEY (collection_id, id)
) STRICT;

-- A document: the stable identity of one source document, its id derived by
-- the import from its source reference (01 §2.1), which no other document of
-- its collection shares.
CREATE TABLE documents (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  UNIQUE (collection_id, source_ref),
  FOREIGN KEY (collection_id, source_id) REFERENCES sources (collection_id, id)
) STRICT;

-- A revision: one exact version of a document's bytes and metadata. Its two
-- digests name the artifacts it pins. `status` is canonicalization's verdict
-- (01 §5); `recorded_at` is when the kernel learned of it.
CREATE TABLE revisions (
  id TEXT PRIMARY KEY NOT NULL,
  document_id TEXT NOT NULL REFERENCES documents (id),
  original_digest TEXT NOT NULL,
  canonical_digest TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('valid', 'valid_with_warnings', 'failed')),
  captured_at TEXT,
  metadata_json TEXT NOT NULL CHECK (json_type(metadata_json) = 'object'),
  recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX revisions_by_document ON revisions (document_id);

-- A revision is immutable once recorded: nothing but its status changes.
CREATE TRIGGER revisions_are_immutable
BEFORE UPDATE OF id, document_id, original_digest, canonical_digest, captured_at,
  metadata_json, recorded_at ON revisions
BEGIN
  SELECT RAISE(ABORT, 'a revision is immutable once recorded: only its status moves');
END;

-- Its status moves only to failed, which it never leaves: a failed revision
-- is never eligible again.
CREATE TRIGGER revisions_move_only_to_failed
BEFORE UPDATE OF status ON revisions
WHEN NEW.status IS NOT OLD.status AND NEW.status IS NOT 'failed'
BEGIN
  SELECT RAISE(ABORT, 'a revision''s status moves only to failed, never from it');
END;

-- The quality gate's one disposition of a revision (01 §4): the outcome, the
-- reasons and rules that gave it, who decided and when.
CREATE TABLE quality_dispositions (
  revision_id TEXT PRIMARY KEY NOT NULL REFERENCES revisions (id),
  disposition TEXT NOT NULL CHECK (disposition IN (
    'accepted', 'accepted_with_warnings', 'needs_reextraction', 'quarantined', 'excluded'
  )),
  reasons_json TEXT NOT NULL CHECK (json_valid(reasons_json)),
  rule_ids TEXT NOT NULL CHECK (json_valid(rule_ids)),
  decided_by TEXT NOT NULL,
  decided_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- Where a revision's content occurs (01 §6): exact duplicates keep every
-- occurrence, each with its own source and source reference.
CREATE TABLE occurrences (
  revision_id TEXT NOT NULL REFERENCES revisions (id),
  collection_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  PRIMARY KEY (revision_id, collection_id, source_id, source_ref),
  FOREIGN KEY (collection_id, source_id) REFERENCES sources (collection_id, id)
) STRICT;

-- The revisions a confirmed shingle Jaccard groups as near duplicates (01 §6):
-- grouped, never deleted.
CREATE TABLE near_dup_groups (
  group_id TEXT NOT NULL,
  revision_id TEXT NOT NULL REFERENCES revisions (id),
  jaccard REAL NOT NULL CHECK (jaccard >= 0 AND jaccard <= 1),
  PRIMARY KEY (group_id, revision_id)
) STRICT;

CREATE INDEX near_dup_groups_by_revision ON near_dup_groups (revision_id);

-- The chunks of a collection under one chunk profile and token counter
-- (01 §7). Its states are the preparation's to name.
CREATE TABLE chunk_sets (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  chunk_profile TEXT NOT NULL,
  counter_contract_id TEXT NOT NULL,
  state TEXT NOT NULL,
  manifest_digest TEXT
) STRICT;

-- A chunk of a chunk set: a passage of one revision, the digest of its
-- prepared input, its token count and its UTF-8 byte span, end exclusive. Its
-- id is unique in its chunk set only: unchanged content keeps its chunk id
-- from one chunk set to the next.
CREATE TABLE chunks (
  chunk_set_id TEXT NOT NULL REFERENCES chunk_sets (id),
  id TEXT NOT NULL,
  revision_id TEXT NOT NULL REFERENCES revisions (id),
  section_id TEXT,
  digest TEXT NOT NULL,
  token_count INTEGER NOT NULL CHECK (token_count >= 0),
  span_start INTEGER NOT NULL CHECK (span_start >= 0),
  span_end INTEGER NOT NULL CHECK (span_end >= span_start),
  PRIMARY KEY (chunk_set_id, id)
) STRICT;

CREATE INDEX chunks_by_revision ON chunks (revision_id);

-- A search generation (plan D9): one build of a collection's projection from
-- one chunk set, with the profiles that represent it. It moves only building,
-- verified, published, retired; its id is never given twice.
CREATE TABLE generations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  chunk_set_id TEXT NOT NULL REFERENCES chunk_sets (id),
  embedding_profile TEXT NOT NULL,
  sparse_profile TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'building'
    CHECK (state IN ('building', 'verified', 'published', 'retired')),
  point_count INTEGER CHECK (point_count >= 0),
  published_at TEXT
) STRICT;

-- At most one published generation per collection, whoever writes.
CREATE UNIQUE INDEX generations_one_published ON generations (collection_id)
WHERE state = 'published';
