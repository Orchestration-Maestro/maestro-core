-- A projection becomes visible only after its unpublished file has been
-- closed, reopened, and verified against the generation's frozen claim set.
DROP TRIGGER graph_projection_receipts_match_attachment;
DROP TRIGGER graph_projection_receipts_never_changed;
DROP TRIGGER graph_projection_receipts_never_deleted;
ALTER TABLE graph_projection_receipts RENAME TO graph_projection_receipts_legacy;
CREATE TABLE graph_projection_receipts (
  generation_id INTEGER PRIMARY KEY NOT NULL REFERENCES generations (id),
  collection_id TEXT NOT NULL REFERENCES collections (id),
  claim_set_id TEXT NOT NULL REFERENCES claim_sets (id),
  file_name TEXT NOT NULL UNIQUE CHECK (
    length(file_name) > 0 AND file_name GLOB '[A-Za-z0-9]*'
    AND file_name NOT GLOB '*[^A-Za-z0-9._-]*'
  ),
  schema_version TEXT NOT NULL CHECK (schema_version IN ('maestro-typed-edges/1', 'maestro-typed-edges/2')),
  knowledge_edge_count INTEGER NOT NULL CHECK (knowledge_edge_count >= 0),
  catalog_dependency_edge_count INTEGER NOT NULL CHECK (catalog_dependency_edge_count >= 0),
  entity_fact_count INTEGER NOT NULL CHECK (entity_fact_count >= 0),
  content_digest TEXT NOT NULL CHECK (length(content_digest) = 64),
  resolution_id TEXT REFERENCES graph_resolutions(id),
  resolver_version TEXT,
  settings_identity TEXT,
  frozen_lock TEXT,
  verified_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (schema_version = 'maestro-typed-edges/1' AND resolution_id IS NULL
      AND resolver_version IS NULL AND settings_identity IS NULL AND frozen_lock IS NULL)
    OR (schema_version = 'maestro-typed-edges/2'
      AND resolution_id IS NOT NULL AND length(resolution_id) = 64
      AND resolution_id NOT GLOB '*[^0-9a-f]*'
      AND resolver_version IS NOT NULL AND resolver_version = 'maestro-exact-resolution/1'
      AND settings_identity IS NOT NULL AND length(settings_identity) = 64
      AND settings_identity NOT GLOB '*[^0-9a-f]*'
      AND frozen_lock IS NOT NULL AND length(frozen_lock) = 64
      AND frozen_lock NOT GLOB '*[^0-9a-f]*')
  )
) STRICT;

INSERT INTO graph_projection_receipts
  (generation_id, collection_id, claim_set_id, file_name, schema_version,
   knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count, content_digest, verified_at)
SELECT generation_id, collection_id, claim_set_id, file_name, schema_version,
   knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count, content_digest, verified_at
FROM graph_projection_receipts_legacy;
DROP TABLE graph_projection_receipts_legacy;

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

CREATE TRIGGER graph_projection_receipts_match_resolution
BEFORE INSERT ON graph_projection_receipts
WHEN NEW.schema_version <> 'maestro-typed-edges/2' OR NOT EXISTS (
  SELECT 1 FROM graph_resolutions r, json_each(r.body, '$.sets') s
  WHERE r.id = NEW.resolution_id AND s.value = NEW.claim_set_id
    AND json_extract(r.body, '$.resolver_version') = NEW.resolver_version
)
BEGIN SELECT RAISE(ABORT, 'projection pins must match a covering frozen resolution'); END;
