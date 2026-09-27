-- Exact identifier membership replaces the tokenized full-text candidate index.
CREATE TABLE chunk_search_identifiers (
  chunk_set_id TEXT NOT NULL,
  identifier TEXT NOT NULL,
  chunk_id TEXT NOT NULL,
  PRIMARY KEY (chunk_set_id, identifier, chunk_id),
  FOREIGN KEY (chunk_set_id, chunk_id)
    REFERENCES chunk_search_inputs (chunk_set_id, chunk_id)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER chunk_search_identifiers_are_never_replaced
BEFORE INSERT ON chunk_search_identifiers
WHEN EXISTS (
  SELECT 1 FROM chunk_search_identifiers
  WHERE chunk_set_id = NEW.chunk_set_id
    AND identifier = NEW.identifier
    AND chunk_id = NEW.chunk_id
)
BEGIN
  SELECT RAISE(ABORT, 'search identifier is never replaced');
END;

CREATE TRIGGER chunk_search_identifiers_never_change
BEFORE UPDATE ON chunk_search_identifiers
BEGIN
  SELECT RAISE(ABORT, 'search identifier never changes');
END;

CREATE TRIGGER chunk_search_identifiers_are_never_deleted
BEFORE DELETE ON chunk_search_identifiers
BEGIN
  SELECT RAISE(ABORT, 'search identifier is never deleted');
END;

DROP TRIGGER chunk_search_inputs_insert;
DROP TABLE chunk_search_fts;
