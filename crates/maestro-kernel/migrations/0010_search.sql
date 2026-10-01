-- Search derivatives of immutable chunks. No text is backfilled here: the
-- publisher records the exact prepared input after reading its digest.
CREATE TABLE chunk_search_inputs (
  rowid INTEGER PRIMARY KEY,
  chunk_set_id TEXT NOT NULL,
  chunk_id TEXT NOT NULL,
  prepared_input TEXT NOT NULL,
  UNIQUE (chunk_set_id, chunk_id),
  FOREIGN KEY (chunk_set_id, chunk_id) REFERENCES chunks (chunk_set_id, id)
) STRICT;

CREATE VIRTUAL TABLE chunk_search_fts USING fts5(
  prepared_input, content='chunk_search_inputs', content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 0'
);

CREATE TRIGGER chunk_search_inputs_insert AFTER INSERT ON chunk_search_inputs
BEGIN
  INSERT INTO chunk_search_fts (rowid, prepared_input)
  VALUES (new.rowid, new.prepared_input);
END;

CREATE TRIGGER chunk_search_inputs_are_never_replaced
BEFORE INSERT ON chunk_search_inputs
WHEN EXISTS (SELECT 1 FROM chunk_search_inputs
             WHERE chunk_set_id = NEW.chunk_set_id AND chunk_id = NEW.chunk_id)
BEGIN
  SELECT RAISE(ABORT, 'prepared search input is never replaced');
END;

CREATE TRIGGER chunk_search_inputs_never_change
BEFORE UPDATE ON chunk_search_inputs
BEGIN
  SELECT RAISE(ABORT, 'prepared search input never changes');
END;

CREATE TRIGGER chunk_search_inputs_are_never_deleted
BEFORE DELETE ON chunk_search_inputs
BEGIN
  SELECT RAISE(ABORT, 'prepared search input is never deleted');
END;

CREATE TABLE chunk_set_members (
  chunk_set_id TEXT NOT NULL REFERENCES chunk_sets (id),
  revision_id TEXT NOT NULL REFERENCES revisions (id),
  representative_revision_id TEXT NOT NULL REFERENCES revisions (id),
  PRIMARY KEY (chunk_set_id, revision_id)
) STRICT;

CREATE TRIGGER chunk_set_members_are_never_replaced
BEFORE INSERT ON chunk_set_members
WHEN EXISTS (SELECT 1 FROM chunk_set_members
             WHERE chunk_set_id = NEW.chunk_set_id AND revision_id = NEW.revision_id)
BEGIN
  SELECT RAISE(ABORT, 'chunk-set member is never replaced');
END;

CREATE TRIGGER chunk_set_members_never_change
BEFORE UPDATE ON chunk_set_members
BEGIN
  SELECT RAISE(ABORT, 'chunk-set member never changes');
END;

CREATE TRIGGER chunk_set_members_are_never_deleted
BEFORE DELETE ON chunk_set_members
BEGIN
  SELECT RAISE(ABORT, 'chunk-set member is never deleted');
END;

CREATE TABLE generation_search (
  generation_id INTEGER PRIMARY KEY REFERENCES generations (id),
  identifier_profile TEXT NOT NULL,
  ready INTEGER NOT NULL DEFAULT 0 CHECK (ready IN (0, 1))
) STRICT;

CREATE TRIGGER generation_search_is_never_replaced
BEFORE INSERT ON generation_search
WHEN EXISTS (SELECT 1 FROM generation_search
             WHERE generation_id = NEW.generation_id)
BEGIN
  SELECT RAISE(ABORT, 'generation search profile is never replaced');
END;

CREATE TRIGGER generation_search_keeps_identity_and_profile
BEFORE UPDATE OF generation_id, identifier_profile ON generation_search
WHEN NEW.generation_id IS NOT OLD.generation_id
  OR NEW.identifier_profile IS NOT OLD.identifier_profile
BEGIN
  SELECT RAISE(ABORT, 'generation search identity and profile never change');
END;

CREATE TRIGGER generation_search_ready_moves_once
BEFORE UPDATE OF ready ON generation_search
WHEN OLD.ready IS NOT 0 OR NEW.ready IS NOT 1
BEGIN
  SELECT RAISE(ABORT, 'generation search readiness moves only from zero to one');
END;

CREATE TRIGGER generation_search_is_never_deleted
BEFORE DELETE ON generation_search
BEGIN
  SELECT RAISE(ABORT, 'generation search marker is never deleted');
END;
