-- Immutable projection inputs, outputs and the one kernel-owned active head.
-- Runtime registration waits for the build-bound producer and native format.
DROP TRIGGER graph_projection_receipts_match_attachment;
DROP TRIGGER graph_projection_receipts_match_resolution;
DROP TRIGGER graph_projection_receipts_never_changed;
DROP TRIGGER graph_projection_receipts_never_deleted;
ALTER TABLE graph_projection_receipts RENAME TO graph_projection_receipts_0030;

CREATE TABLE graph_projection_builds (
  build_id INTEGER PRIMARY KEY AUTOINCREMENT CHECK (build_id > 0),
  generation_id INTEGER NOT NULL REFERENCES generations(id),
  collection_id TEXT NOT NULL REFERENCES collections(id),
  claim_set_id TEXT NOT NULL REFERENCES claim_sets(id),
  project_job_id TEXT UNIQUE REFERENCES jobs(id),
  expected_active_build_id INTEGER,
  schema_version TEXT NOT NULL CHECK (schema_version IN (
    'maestro-typed-edges/1', 'maestro-typed-edges/2', 'maestro-typed-edges/3'
  )),
  resolution_id TEXT REFERENCES graph_resolutions(id),
  resolver_version TEXT,
  settings_identity TEXT,
  frozen_lock TEXT,
  UNIQUE (generation_id, build_id),
  FOREIGN KEY (generation_id, expected_active_build_id)
    REFERENCES graph_projection_builds(generation_id, build_id),
  CHECK (project_job_id IS NOT NULL OR expected_active_build_id IS NULL),
  CHECK (project_job_id IS NULL OR schema_version = 'maestro-typed-edges/3'),
  CHECK (
    (schema_version = 'maestro-typed-edges/1'
      AND resolution_id IS NULL AND resolver_version IS NULL
      AND settings_identity IS NULL AND frozen_lock IS NULL)
    OR (schema_version IN ('maestro-typed-edges/2', 'maestro-typed-edges/3')
      AND resolution_id IS NOT NULL AND length(resolution_id) = 64
      AND resolution_id NOT GLOB '*[^0-9a-f]*'
      AND resolver_version IS NOT NULL
      AND resolver_version = 'maestro-exact-resolution/1'
      AND settings_identity IS NOT NULL AND length(settings_identity) = 64
      AND settings_identity NOT GLOB '*[^0-9a-f]*'
      AND frozen_lock IS NOT NULL AND length(frozen_lock) = 64
      AND frozen_lock NOT GLOB '*[^0-9a-f]*')
  )
) STRICT;

CREATE TABLE graph_projection_receipts (
  build_id INTEGER PRIMARY KEY NOT NULL
    REFERENCES graph_projection_builds(build_id),
  file_name TEXT NOT NULL UNIQUE CHECK (
    length(file_name) > 0 AND file_name GLOB '[A-Za-z0-9]*'
    AND file_name NOT GLOB '*[^A-Za-z0-9._-]*'
  ),
  knowledge_edge_count INTEGER NOT NULL CHECK (knowledge_edge_count >= 0),
  catalog_dependency_edge_count INTEGER NOT NULL
    CHECK (catalog_dependency_edge_count >= 0),
  entity_fact_count INTEGER NOT NULL CHECK (entity_fact_count >= 0),
  content_digest TEXT NOT NULL CHECK (length(content_digest) = 64),
  verified_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE graph_projection_active (
  generation_id INTEGER PRIMARY KEY NOT NULL REFERENCES generations(id),
  build_id INTEGER NOT NULL REFERENCES graph_projection_receipts(build_id),
  FOREIGN KEY (generation_id, build_id)
    REFERENCES graph_projection_builds(generation_id, build_id)
) STRICT;

-- Historical records are copied before runtime admission guards are installed.
INSERT INTO graph_projection_builds
  (build_id, generation_id, collection_id, claim_set_id, schema_version,
   resolution_id, resolver_version, settings_identity, frozen_lock)
SELECT generation_id, generation_id, collection_id, claim_set_id, schema_version,
       resolution_id, resolver_version, settings_identity, frozen_lock
FROM graph_projection_receipts_0030;
INSERT INTO graph_projection_receipts
  (build_id, file_name, knowledge_edge_count, catalog_dependency_edge_count,
   entity_fact_count, content_digest, verified_at)
SELECT generation_id, file_name, knowledge_edge_count, catalog_dependency_edge_count,
       entity_fact_count, content_digest, verified_at
FROM graph_projection_receipts_0030;
INSERT INTO graph_projection_active (generation_id, build_id)
SELECT generation_id, generation_id FROM graph_projection_receipts_0030;

-- Fail the enclosing migration transaction if any copied byte or key differs.
CREATE TEMP TABLE graph_projection_copy_check (valid INTEGER CHECK (valid = 1));
INSERT INTO graph_projection_copy_check
SELECT
  (SELECT count(*) FROM graph_projection_receipts_0030) =
    (SELECT count(*) FROM graph_projection_builds)
  AND (SELECT count(*) FROM graph_projection_receipts_0030) =
    (SELECT count(*) FROM graph_projection_receipts)
  AND (SELECT count(*) FROM graph_projection_receipts_0030) =
    (SELECT count(*) FROM graph_projection_active)
  AND NOT EXISTS (
    SELECT generation_id, collection_id, claim_set_id, file_name, schema_version,
           knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count,
           content_digest, resolution_id, resolver_version, settings_identity,
           frozen_lock, verified_at
    FROM graph_projection_receipts_0030
    EXCEPT
    SELECT b.generation_id, b.collection_id, b.claim_set_id, r.file_name,
           b.schema_version, r.knowledge_edge_count, r.catalog_dependency_edge_count,
           r.entity_fact_count, r.content_digest, b.resolution_id, b.resolver_version,
           b.settings_identity, b.frozen_lock, r.verified_at
    FROM graph_projection_builds b
    JOIN graph_projection_receipts r ON r.build_id = b.build_id
    JOIN graph_projection_active a
      ON a.generation_id = b.generation_id AND a.build_id = b.build_id
    WHERE b.build_id = b.generation_id AND b.project_job_id IS NULL
      AND b.expected_active_build_id IS NULL
  );
INSERT INTO graph_projection_copy_check
SELECT NOT EXISTS (SELECT 1 FROM pragma_foreign_key_check);
DROP TABLE graph_projection_copy_check;
DROP TABLE graph_projection_receipts_0030;

CREATE TRIGGER graph_projection_builds_admit
BEFORE INSERT ON graph_projection_builds
WHEN NEW.project_job_id IS NULL OR NEW.schema_version <> 'maestro-typed-edges/3'
  OR NOT EXISTS (
    SELECT 1 FROM generations g
    JOIN graph_attachments a ON a.generation_id = g.id
    LEFT JOIN graph_projection_active h ON h.generation_id = g.id
    LEFT JOIN graph_projection_builds p ON p.build_id = h.build_id
    LEFT JOIN graph_projection_receipts r ON r.build_id = p.build_id
    WHERE g.id = NEW.generation_id AND g.collection_id = NEW.collection_id
      AND a.claim_set_id = NEW.claim_set_id
      AND (
        (NEW.expected_active_build_id IS NULL AND h.build_id IS NULL
          AND g.state = 'verified')
        OR (NEW.expected_active_build_id IS NOT NULL
          AND h.build_id IS NEW.expected_active_build_id
          AND g.state IN ('verified', 'published') AND r.build_id IS NOT NULL
          AND p.generation_id = NEW.generation_id
          AND (p.schema_version = 'maestro-typed-edges/1'
            OR (p.claim_set_id = NEW.claim_set_id
              AND p.resolution_id = NEW.resolution_id
              AND p.resolver_version = NEW.resolver_version)))
      )
  ) OR NOT EXISTS (
    SELECT 1 FROM graph_resolutions r, json_each(r.body, '$.sets') s
    WHERE r.id = NEW.resolution_id AND s.value = NEW.claim_set_id
      AND json_extract(r.body, '$.resolver_version') = NEW.resolver_version
  )
BEGIN SELECT RAISE(ABORT, 'projection build must match frozen inputs and the active head'); END;

-- BEFORE INSERT also blocks REPLACE when recursive delete triggers are disabled.
CREATE TRIGGER graph_projection_builds_never_replaced
BEFORE INSERT ON graph_projection_builds
WHEN EXISTS (SELECT 1 FROM graph_projection_builds
  WHERE build_id = NEW.build_id
    OR (NEW.project_job_id IS NOT NULL AND project_job_id = NEW.project_job_id))
BEGIN SELECT RAISE(ABORT, 'projection builds cannot be replaced'); END;
CREATE TRIGGER graph_projection_builds_never_changed
BEFORE UPDATE ON graph_projection_builds
BEGIN SELECT RAISE(ABORT, 'projection build inputs are immutable'); END;
CREATE TRIGGER graph_projection_builds_never_deleted
BEFORE DELETE ON graph_projection_builds
BEGIN SELECT RAISE(ABORT, 'projection builds are retained'); END;

CREATE TRIGGER graph_projection_receipts_match_build
BEFORE INSERT ON graph_projection_receipts
WHEN NEW.content_digest GLOB '*[^0-9a-f]*' OR NOT EXISTS (
  SELECT 1 FROM graph_projection_builds b
  JOIN generations g ON g.id = b.generation_id
  JOIN graph_attachments a ON a.generation_id = g.id
  JOIN graph_resolutions resolution ON resolution.id = b.resolution_id
  LEFT JOIN graph_projection_active h ON h.generation_id = g.id
  LEFT JOIN graph_projection_builds p ON p.build_id = h.build_id
  LEFT JOIN graph_projection_receipts r ON r.build_id = p.build_id
  WHERE b.build_id = NEW.build_id AND b.project_job_id IS NOT NULL
    AND b.schema_version = 'maestro-typed-edges/3'
    AND g.collection_id = b.collection_id AND a.claim_set_id = b.claim_set_id
    AND json_extract(resolution.body, '$.resolver_version') = b.resolver_version
    AND EXISTS (SELECT 1 FROM json_each(resolution.body, '$.sets') s
      WHERE s.value = b.claim_set_id)
    AND (
      (b.expected_active_build_id IS NULL AND h.build_id IS NULL
        AND g.state = 'verified')
      OR (b.expected_active_build_id IS NOT NULL
        AND h.build_id IS b.expected_active_build_id
        AND g.state IN ('verified', 'published') AND r.build_id IS NOT NULL
        AND p.generation_id = b.generation_id
        AND (p.schema_version = 'maestro-typed-edges/1'
          OR (p.claim_set_id = b.claim_set_id
            AND p.resolution_id = b.resolution_id
            AND p.resolver_version = b.resolver_version)))
    )
    AND NEW.knowledge_edge_count = (
      SELECT count(*) FROM claim_set_members m JOIN claims c ON c.id = m.claim_id
      WHERE m.claim_set_id = b.claim_set_id AND c.object_kind IS NOT NULL)
    AND NEW.entity_fact_count = (
      SELECT count(*) FROM claim_set_members m JOIN claims c ON c.id = m.claim_id
      WHERE m.claim_set_id = b.claim_set_id AND c.object_type IS NOT NULL)
)
BEGIN SELECT RAISE(ABORT, 'projection receipt must match an eligible frozen build and counts'); END;
CREATE TRIGGER graph_projection_receipts_never_replaced
BEFORE INSERT ON graph_projection_receipts
WHEN EXISTS (SELECT 1 FROM graph_projection_receipts
  WHERE build_id = NEW.build_id OR file_name = NEW.file_name)
BEGIN SELECT RAISE(ABORT, 'projection receipts cannot be replaced'); END;
CREATE TRIGGER graph_projection_receipts_never_changed
BEFORE UPDATE ON graph_projection_receipts
BEGIN SELECT RAISE(ABORT, 'projection receipts are immutable'); END;
CREATE TRIGGER graph_projection_receipts_never_deleted
BEFORE DELETE ON graph_projection_receipts
BEGIN SELECT RAISE(ABORT, 'projection receipts are retained'); END;

CREATE TRIGGER graph_projection_active_admit
BEFORE INSERT ON graph_projection_active
WHEN NOT EXISTS (
  SELECT 1 FROM graph_projection_builds b
  JOIN graph_projection_receipts r ON r.build_id = b.build_id
  JOIN generations g ON g.id = b.generation_id
  WHERE b.build_id = NEW.build_id AND b.generation_id = NEW.generation_id
    AND b.project_job_id IS NOT NULL AND b.schema_version = 'maestro-typed-edges/3'
    AND b.expected_active_build_id IS NULL AND g.state = 'verified'
)
BEGIN SELECT RAISE(ABORT, 'initial projection head requires a verified receipted build'); END;
CREATE TRIGGER graph_projection_active_advance
BEFORE UPDATE ON graph_projection_active
WHEN NEW.generation_id <> OLD.generation_id OR NEW.build_id = OLD.build_id
  OR NOT EXISTS (
    SELECT 1 FROM graph_projection_builds b
    JOIN graph_projection_receipts r ON r.build_id = b.build_id
    JOIN generations g ON g.id = b.generation_id
    WHERE b.build_id = NEW.build_id AND b.generation_id = OLD.generation_id
      AND b.project_job_id IS NOT NULL AND b.schema_version = 'maestro-typed-edges/3'
      AND b.expected_active_build_id IS OLD.build_id
      AND g.state IN ('verified', 'published')
  )
BEGIN SELECT RAISE(ABORT, 'projection head must advance from its exact predecessor'); END;
CREATE TRIGGER graph_projection_active_never_deleted
BEFORE DELETE ON graph_projection_active
BEGIN SELECT RAISE(ABORT, 'projection active heads are retained'); END;
CREATE TRIGGER graph_projection_active_never_replaced
BEFORE INSERT ON graph_projection_active
WHEN EXISTS (SELECT 1 FROM graph_projection_active WHERE generation_id = NEW.generation_id)
BEGIN SELECT RAISE(ABORT, 'projection active heads cannot be replaced'); END;
