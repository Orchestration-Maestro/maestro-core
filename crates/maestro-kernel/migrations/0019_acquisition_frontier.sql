-- Kernel-owned frontier: source ownership reuses jobs and their journal.
-- A source keeps one logical job through restarts and writer takeovers.
CREATE TABLE acquisition_sources (
  source TEXT PRIMARY KEY NOT NULL,
  job TEXT NOT NULL UNIQUE REFERENCES jobs(id)
) STRICT;

-- Authorization and representation contexts are independent identity components.
-- Capture NULL means durable unfinished work, even after a crashed dispatch.
CREATE TABLE acquisition_frontier (
  id TEXT PRIMARY KEY NOT NULL,
  source TEXT NOT NULL REFERENCES acquisition_sources(source),
  job TEXT NOT NULL REFERENCES jobs(id),
  fetch_identity TEXT NOT NULL,
  authorization_context TEXT NOT NULL,
  representation_profile TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  lease_epoch INTEGER NOT NULL DEFAULT 0 CHECK (lease_epoch = attempts),
  writer_epoch INTEGER NOT NULL DEFAULT 0 CHECK (writer_epoch >= 0),
  lease_holder TEXT,
  lease_expires TEXT,
  capture TEXT REFERENCES artifacts(digest),
  capture_generation INTEGER NOT NULL DEFAULT 0 CHECK (capture_generation >= 0),
  observed_ms INTEGER CHECK (observed_ms >= 0),
  work_verified INTEGER GENERATED ALWAYS AS (capture IS NOT NULL) STORED,
  work_observed INTEGER GENERATED ALWAYS AS
    (CASE WHEN capture IS NULL THEN 0 ELSE COALESCE(observed_ms, 0) END) STORED,
  UNIQUE (source, fetch_identity, authorization_context, representation_profile),
  CHECK ((lease_epoch = 0) = (lease_holder IS NULL)),
  CHECK ((lease_holder IS NULL) = (lease_expires IS NULL)),
  CHECK (lease_epoch = 0 OR writer_epoch > 0),
  CHECK (capture IS NULL OR lease_epoch > 0)
) STRICT;

CREATE INDEX acquisition_frontier_source_page ON acquisition_frontier(source, id);

-- Pending-first, then oldest verified observation; one indexed cursor range.
CREATE INDEX acquisition_frontier_work_page ON acquisition_frontier
(source, work_verified, work_observed, id);

-- Only an explicit refresh can change the acknowledged pointer.
-- All capture generations and their artifact pins remain immutable.
CREATE TRIGGER acquisition_captures_never_change
BEFORE UPDATE ON acquisition_frontier
WHEN OLD.capture IS NOT NULL AND NOT (
  NEW.capture IS NULL AND NEW.capture_generation = OLD.capture_generation + 1
) AND NEW.capture IS NOT OLD.capture
BEGIN
  SELECT RAISE(ABORT, 'an acknowledged frontier capture never changes');
END;
