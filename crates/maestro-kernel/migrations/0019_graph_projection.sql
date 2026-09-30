-- A projection becomes visible only after its unpublished file has been
-- closed, reopened, and verified against the generation's frozen claim set.
CREATE TABLE graph_projection_receipts (
  generation_id INTEGER PRIMARY KEY NOT NULL REFERENCES generations (id),
  collection_id TEXT NOT NULL REFERENCES collections (id),
  claim_set_id TEXT NOT NULL REFERENCES claim_sets (id),
  file_name TEXT NOT NULL UNIQUE CHECK (
    length(file_name) > 0 AND file_name GLOB '[A-Za-z0-9]*'
    AND file_name NOT GLOB '*[^A-Za-z0-9._-]*'
  ),
  schema_version TEXT NOT NULL CHECK (schema_version = 'maestro-typed-edges/1'),
  knowledge_edge_count INTEGER NOT NULL CHECK (knowledge_edge_count >= 0),
  catalog_dependency_edge_count INTEGER NOT NULL CHECK (catalog_dependency_edge_count >= 0),
  entity_fact_count INTEGER NOT NULL CHECK (entity_fact_count >= 0),
  content_digest TEXT NOT NULL CHECK (length(content_digest) = 64),
  verified_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TRIGGER graph_projection_receipts_match_attachment
BEFORE INSERT ON graph_projection_receipts
WHEN NOT EXISTS (
  SELECT 1 FROM generations g
  JOIN graph_attachments a ON a.generation_id = g.id
  WHERE g.id = NEW.generation_id AND g.collection_id = NEW.collection_id
    AND g.state = 'verified' AND a.claim_set_id = NEW.claim_set_id
    AND NEW.knowledge_edge_count = (
      SELECT count(*) FROM claim_set_members m JOIN claims c ON c.id = m.claim_id
      WHERE m.claim_set_id = a.claim_set_id AND c.object_kind IS NOT NULL
    )
    AND NEW.entity_fact_count = (
      SELECT count(*) FROM claim_set_members m JOIN claims c ON c.id = m.claim_id
      WHERE m.claim_set_id = a.claim_set_id AND c.object_type IS NOT NULL
    )
)
BEGIN
  SELECT RAISE(ABORT, 'projection receipt must match a verified generation attachment and its claim counts');
END;

CREATE TRIGGER graph_projection_receipts_never_changed
BEFORE UPDATE ON graph_projection_receipts
BEGIN SELECT RAISE(ABORT, 'projection readiness never changes'); END;

CREATE TRIGGER graph_projection_receipts_never_deleted
BEFORE DELETE ON graph_projection_receipts
BEGIN SELECT RAISE(ABORT, 'projection readiness is retained'); END;
