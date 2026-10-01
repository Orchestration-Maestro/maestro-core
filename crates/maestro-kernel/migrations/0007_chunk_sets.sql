-- The guards of the chunk sets and their chunks (docs/architecture/01 §7;
-- T023), which `0004_documents` left to the kernel's functions: whoever
-- writes, a chunk set now keeps its states, its moves and its identity, and a
-- complete set keeps its chunks. A chunk set is inserted building, without a
-- manifest, and moves only from building, to complete with the manifest it
-- pins, or to failed without one; both are final, so the three states are
-- the only ones a set ever holds. It keeps its id, collection, chunk profile
-- and counter, and is never replaced nor deleted. A chunk enters only a
-- building set, for a revision of the set's collection, and never changes,
-- nor is replaced or deleted; a chunk whose set or revision is missing is
-- left to the foreign keys. INSERT OR REPLACE removes the row it conflicts
-- with without firing a delete trigger, so an insert that would take the
-- place of a set or a chunk is refused before SQLite resolves the conflict.

CREATE TRIGGER chunk_sets_begin_building
BEFORE INSERT ON chunk_sets
WHEN NEW.state IS NOT 'building' OR NEW.manifest_digest IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'a chunk set begins building, without a manifest');
END;

CREATE TRIGGER chunk_sets_are_never_replaced
BEFORE INSERT ON chunk_sets
WHEN EXISTS (SELECT 1 FROM chunk_sets WHERE id = NEW.id)
BEGIN
  SELECT RAISE(ABORT, 'a chunk set is never replaced: its id names it for good');
END;

CREATE TRIGGER chunk_sets_keep_their_identity
BEFORE UPDATE OF id, collection_id, chunk_profile, counter_contract_id ON chunk_sets
WHEN NEW.id IS NOT OLD.id OR NEW.collection_id IS NOT OLD.collection_id
  OR NEW.chunk_profile IS NOT OLD.chunk_profile
  OR NEW.counter_contract_id IS NOT OLD.counter_contract_id
BEGIN
  SELECT RAISE(ABORT, 'a chunk set keeps its id, collection, chunk profile and counter');
END;

CREATE TRIGGER chunk_sets_move_only_from_building
BEFORE UPDATE OF state, manifest_digest ON chunk_sets
WHEN OLD.state IS NOT 'building'
  OR NOT ((NEW.state IS 'complete' AND NEW.manifest_digest IS NOT NULL)
    OR (NEW.state IS 'failed' AND NEW.manifest_digest IS NULL))
BEGIN
  SELECT RAISE(ABORT,
    'a chunk set moves only from building, to complete with its manifest or to failed without one');
END;

CREATE TRIGGER chunk_sets_are_never_deleted
BEFORE DELETE ON chunk_sets
BEGIN
  SELECT RAISE(ABORT, 'a chunk set is never deleted: a generation may name it');
END;

CREATE TRIGGER chunks_enter_a_building_set_of_their_collection
BEFORE INSERT ON chunks
WHEN EXISTS (SELECT 1 FROM chunk_sets WHERE id = NEW.chunk_set_id)
  AND EXISTS (SELECT 1 FROM revisions WHERE id = NEW.revision_id)
  AND NOT EXISTS (
  SELECT 1 FROM chunk_sets
  JOIN revisions ON revisions.id = NEW.revision_id
  JOIN documents ON documents.id = revisions.document_id
  WHERE chunk_sets.id = NEW.chunk_set_id AND chunk_sets.state = 'building'
    AND documents.collection_id = chunk_sets.collection_id)
BEGIN
  SELECT RAISE(ABORT,
    'a chunk enters only a building chunk set, for a revision of the set''s collection');
END;

CREATE TRIGGER chunks_are_never_replaced
BEFORE INSERT ON chunks
WHEN EXISTS (SELECT 1 FROM chunks WHERE chunk_set_id = NEW.chunk_set_id AND id = NEW.id)
BEGIN
  SELECT RAISE(ABORT, 'a chunk is never replaced: its set holds it as it was counted');
END;

CREATE TRIGGER chunks_never_change
BEFORE UPDATE ON chunks
BEGIN
  SELECT RAISE(ABORT, 'a chunk never changes: its set holds it as it was counted');
END;

CREATE TRIGGER chunks_are_never_deleted
BEFORE DELETE ON chunks
BEGIN
  SELECT RAISE(ABORT, 'a chunk is never deleted: its set holds it as it was counted');
END;
