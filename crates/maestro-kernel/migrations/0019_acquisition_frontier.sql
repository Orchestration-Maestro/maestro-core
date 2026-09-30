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
  UNIQUE (source, fetch_identity, authorization_context, representation_profile),
  CHECK ((lease_epoch = 0) = (lease_holder IS NULL)),
  CHECK ((lease_holder IS NULL) = (lease_expires IS NULL)),
  CHECK (lease_epoch = 0 OR writer_epoch > 0),
  CHECK (capture IS NULL OR lease_epoch > 0)
) STRICT;

CREATE INDEX acquisition_frontier_source_page ON acquisition_frontier(source, id);

-- Acknowledgements and their pins are immutable, not an overwritable checkpoint.
CREATE TRIGGER acquisition_captures_never_change
BEFORE UPDATE ON acquisition_frontier
WHEN OLD.capture IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'an acknowledged frontier capture never changes');
END;
