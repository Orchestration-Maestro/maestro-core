CREATE UNIQUE INDEX generations_representation_identity
ON generations(collection_id,chunk_set_id,id);

CREATE TABLE representation_sets (
  collection_id TEXT NOT NULL,
  chunk_set_id TEXT NOT NULL,
  id TEXT NOT NULL,
  profile_digest TEXT NOT NULL,
  embedding_profile TEXT NOT NULL,
  sparse_profile TEXT NOT NULL,
  layout TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('building','complete','failed')),
  PRIMARY KEY(collection_id,chunk_set_id,id),
  FOREIGN KEY(collection_id,chunk_set_id) REFERENCES chunk_set_profiles(collection_id,chunk_set_id)
) STRICT;

-- Representation members live in one immutable CAS shard per revision.
CREATE TABLE representation_revisions (
  collection_id TEXT NOT NULL,
  chunk_set_id TEXT NOT NULL,
  representation_set_id TEXT NOT NULL,
  revision_id TEXT NOT NULL,
  shard_digest TEXT NOT NULL REFERENCES artifacts(digest),
  PRIMARY KEY(collection_id,chunk_set_id,representation_set_id,revision_id),
  FOREIGN KEY(collection_id,chunk_set_id,representation_set_id)
    REFERENCES representation_sets(collection_id,chunk_set_id,id),
  FOREIGN KEY(collection_id,chunk_set_id,revision_id)
    REFERENCES revision_unit_graphs(collection_id,chunk_set_id,revision_id)
) STRICT;

CREATE TABLE generation_representations (
  generation_id INTEGER PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL,
  chunk_set_id TEXT NOT NULL,
  representation_set_id TEXT NOT NULL,
  FOREIGN KEY(collection_id,chunk_set_id,generation_id)
    REFERENCES generations(collection_id,chunk_set_id,id),
  FOREIGN KEY(collection_id,chunk_set_id,representation_set_id)
    REFERENCES representation_sets(collection_id,chunk_set_id,id)
) STRICT;

CREATE TRIGGER representation_sets_begin_building
BEFORE INSERT ON representation_sets
WHEN NEW.state IS NOT 'building' OR NOT EXISTS (
  SELECT 1 FROM chunk_set_profiles WHERE collection_id=NEW.collection_id
    AND chunk_set_id=NEW.chunk_set_id AND profile_digest=NEW.profile_digest)
BEGIN SELECT RAISE(ABORT,'representation begins building with matching profile'); END;

CREATE TRIGGER representation_revisions_building
BEFORE INSERT ON representation_revisions
WHEN NOT EXISTS (
  SELECT 1 FROM representation_sets WHERE collection_id=NEW.collection_id
    AND chunk_set_id=NEW.chunk_set_id AND id=NEW.representation_set_id AND state='building')
BEGIN SELECT RAISE(ABORT,'representation shards insert only while building'); END;

CREATE TRIGGER representation_revisions_never_replaced
BEFORE INSERT ON representation_revisions
WHEN EXISTS (SELECT 1 FROM representation_revisions WHERE collection_id=NEW.collection_id
  AND chunk_set_id=NEW.chunk_set_id AND representation_set_id=NEW.representation_set_id
  AND revision_id=NEW.revision_id)
BEGIN SELECT RAISE(ABORT,'immutable shard cannot be replaced'); END;

CREATE TRIGGER representation_revisions_no_update
BEFORE UPDATE ON representation_revisions
BEGIN SELECT RAISE(ABORT,'immutable shard cannot change'); END;

CREATE TRIGGER representation_revisions_no_delete
BEFORE DELETE ON representation_revisions
BEGIN SELECT RAISE(ABORT,'immutable shard cannot change'); END;

CREATE TRIGGER representation_sets_keep_identity
BEFORE UPDATE OF collection_id,chunk_set_id,id,profile_digest,
  embedding_profile,sparse_profile,layout ON representation_sets
BEGIN SELECT RAISE(ABORT,'representation identity never changes'); END;

CREATE TRIGGER representation_sets_move_once
BEFORE UPDATE OF state ON representation_sets
WHEN OLD.state IS NOT 'building' OR NEW.state NOT IN ('complete','failed')
BEGIN SELECT RAISE(ABORT,'representation moves only building to complete or failed'); END;

CREATE TRIGGER generation_representations_matching_complete
BEFORE INSERT ON generation_representations
WHEN NOT EXISTS (
  SELECT 1 FROM representation_sets AS r
  JOIN generations AS g ON g.id=NEW.generation_id
  JOIN chunk_sets AS c ON c.id=r.chunk_set_id
  WHERE r.collection_id=NEW.collection_id AND r.chunk_set_id=NEW.chunk_set_id
    AND r.id=NEW.representation_set_id AND r.state='complete' AND c.state='complete'
    AND g.collection_id=r.collection_id AND g.chunk_set_id=r.chunk_set_id
    AND g.embedding_profile=r.embedding_profile AND g.sparse_profile=r.sparse_profile
    AND g.state='building')
BEGIN SELECT RAISE(ABORT,'generation requires matching complete representation'); END;

CREATE TRIGGER representation_sets_never_replaced
BEFORE INSERT ON representation_sets
WHEN EXISTS (SELECT 1 FROM representation_sets WHERE collection_id=NEW.collection_id AND chunk_set_id=NEW.chunk_set_id AND id=NEW.id)
BEGIN SELECT RAISE(ABORT,'immutable record cannot be replaced'); END;

CREATE TRIGGER representation_sets_no_delete
BEFORE DELETE ON representation_sets
BEGIN SELECT RAISE(ABORT,'immutable record cannot change'); END;

CREATE TRIGGER generation_representations_never_replaced
BEFORE INSERT ON generation_representations
WHEN EXISTS (SELECT 1 FROM generation_representations WHERE generation_id=NEW.generation_id)
BEGIN SELECT RAISE(ABORT,'immutable record cannot be replaced'); END;

CREATE TRIGGER generation_representations_no_update
BEFORE UPDATE ON generation_representations
BEGIN SELECT RAISE(ABORT,'immutable record cannot change'); END;

CREATE TRIGGER generation_representations_no_delete
BEFORE DELETE ON generation_representations
BEGIN SELECT RAISE(ABORT,'immutable record cannot change'); END;
