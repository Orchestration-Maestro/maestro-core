-- Graph content is one immutable CAS artifact per revision. SQL indexes only
-- candidate revision lookups, never parts, siblings or transitive ancestry.
CREATE UNIQUE INDEX documents_graph_identity ON documents(collection_id,id);
CREATE UNIQUE INDEX revisions_graph_identity ON revisions(document_id,id);

CREATE TABLE chunk_set_profiles (
  collection_id TEXT NOT NULL,
  chunk_set_id TEXT NOT NULL,
  profile_name TEXT NOT NULL CHECK(profile_name='mapped-structural-chunks/4'),
  profile_digest TEXT NOT NULL,
  preparation_name TEXT NOT NULL CHECK(preparation_name='canonical-context-parts/v3'),
  preparation_digest TEXT NOT NULL,
  counter_contract TEXT NOT NULL,
  rank_policy TEXT NOT NULL CHECK(rank_policy IN ('complete_ideas','v2_unit')),
  PRIMARY KEY(collection_id,chunk_set_id),
  FOREIGN KEY(collection_id,chunk_set_id) REFERENCES chunk_sets(collection_id,id)
) STRICT;

CREATE TABLE revision_unit_graphs (
  collection_id TEXT NOT NULL,
  chunk_set_id TEXT NOT NULL,
  revision_id TEXT NOT NULL,
  document_id TEXT NOT NULL,
  graph_digest TEXT NOT NULL REFERENCES artifacts(digest),
  mapping_digest TEXT NOT NULL REFERENCES artifacts(digest),
  schema_version TEXT NOT NULL CHECK(schema_version='maestro-unit-graph/1'),
  original_markdown_digest TEXT NOT NULL REFERENCES artifacts(digest),
  PRIMARY KEY(collection_id,chunk_set_id,revision_id),
  FOREIGN KEY(collection_id,chunk_set_id) REFERENCES chunk_set_profiles(collection_id,chunk_set_id),
  FOREIGN KEY(collection_id,document_id) REFERENCES documents(collection_id,id),
  FOREIGN KEY(document_id,revision_id) REFERENCES revisions(document_id,id)
) STRICT;

CREATE TRIGGER chunk_set_profiles_building
BEFORE INSERT ON chunk_set_profiles
WHEN NOT EXISTS (SELECT 1 FROM chunk_sets WHERE id=NEW.chunk_set_id
  AND collection_id=NEW.collection_id AND state='building'
  AND chunk_profile=NEW.profile_name AND counter_contract_id=NEW.counter_contract)
BEGIN SELECT RAISE(ABORT,'graph profile requires matching building chunk set'); END;

CREATE TRIGGER revision_unit_graphs_building
BEFORE INSERT ON revision_unit_graphs
WHEN NOT EXISTS (SELECT 1 FROM chunk_sets JOIN revisions ON revisions.id=NEW.revision_id
  WHERE chunk_sets.id=NEW.chunk_set_id AND chunk_sets.collection_id=NEW.collection_id
  AND chunk_sets.state='building' AND revisions.status!='failed'
  AND revisions.original_digest=NEW.original_markdown_digest)
BEGIN SELECT RAISE(ABORT,'graph requires building set and matching source digest'); END;

CREATE TRIGGER chunk_set_profiles_never_replaced
BEFORE INSERT ON chunk_set_profiles
WHEN EXISTS (SELECT 1 FROM chunk_set_profiles WHERE collection_id=NEW.collection_id AND chunk_set_id=NEW.chunk_set_id)
BEGIN SELECT RAISE(ABORT,'immutable record cannot be replaced'); END;

CREATE TRIGGER chunk_set_profiles_no_update
BEFORE UPDATE ON chunk_set_profiles
BEGIN SELECT RAISE(ABORT,'immutable record cannot change'); END;

CREATE TRIGGER chunk_set_profiles_no_delete
BEFORE DELETE ON chunk_set_profiles
BEGIN SELECT RAISE(ABORT,'immutable record cannot change'); END;

CREATE TRIGGER revision_unit_graphs_never_replaced
BEFORE INSERT ON revision_unit_graphs
WHEN EXISTS (SELECT 1 FROM revision_unit_graphs WHERE collection_id=NEW.collection_id AND chunk_set_id=NEW.chunk_set_id AND revision_id=NEW.revision_id)
BEGIN SELECT RAISE(ABORT,'immutable record cannot be replaced'); END;

CREATE TRIGGER revision_unit_graphs_no_update
BEFORE UPDATE ON revision_unit_graphs
BEGIN SELECT RAISE(ABORT,'immutable record cannot change'); END;

CREATE TRIGGER revision_unit_graphs_no_delete
BEFORE DELETE ON revision_unit_graphs
BEGIN SELECT RAISE(ABORT,'immutable record cannot change'); END;

-- The graph seals this revision's exact chunk memberships even while other
-- revisions are still being appended to the same building chunk set.
CREATE TRIGGER chunks_cannot_extend_recorded_graph
BEFORE INSERT ON chunks
WHEN EXISTS (SELECT 1 FROM revision_unit_graphs
  WHERE chunk_set_id=NEW.chunk_set_id AND revision_id=NEW.revision_id)
BEGIN SELECT RAISE(ABORT,'recorded graph freezes revision chunk membership'); END;

CREATE INDEX revision_unit_graphs_by_set_revision
ON revision_unit_graphs(chunk_set_id,revision_id);
