-- The guards of the documents (docs/architecture/01 §2.1), which
-- `0004_documents` left to the kernel's functions: whoever writes, a document
-- keeps the id, collection, source and source reference it was first
-- recorded with, so neither it nor its revisions, nor their chunks in a
-- complete chunk set, move to another collection. Setting a column to its
-- own value changes nothing and passes. INSERT OR REPLACE removes the row it
-- conflicts with without firing a delete trigger, and an upsert updates it,
-- so an insert that would take the id or the source reference of a document
-- is refused before SQLite resolves the conflict.

CREATE TRIGGER documents_keep_their_identity
BEFORE UPDATE OF id, collection_id, source_id, source_ref ON documents
WHEN NEW.id IS NOT OLD.id OR NEW.collection_id IS NOT OLD.collection_id
  OR NEW.source_id IS NOT OLD.source_id OR NEW.source_ref IS NOT OLD.source_ref
BEGIN
  SELECT RAISE(ABORT, 'a document keeps its id, collection, source and source reference');
END;

CREATE TRIGGER documents_are_never_replaced
BEFORE INSERT ON documents
WHEN EXISTS (SELECT 1 FROM documents WHERE id = NEW.id)
  OR EXISTS (SELECT 1 FROM documents
    WHERE collection_id = NEW.collection_id AND source_ref = NEW.source_ref)
BEGIN
  SELECT RAISE(ABORT,
    'a document is never replaced: no insert takes its id or its source reference');
END;
