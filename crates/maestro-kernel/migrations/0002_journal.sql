-- The journal (plan D3): every change the kernel makes, as an event in one
-- append-only table. An event's ID is a ULID, its sequence the next number of
-- its stream, taken in the transaction that inserts it, and its time RFC 3339
-- in UTC, as the kernel's other tables write theirs; its data is JSON text.
CREATE TABLE events (
  id TEXT PRIMARY KEY NOT NULL,
  stream TEXT NOT NULL,
  sequence INTEGER NOT NULL CHECK (sequence >= 1),
  type TEXT NOT NULL,
  subject TEXT NOT NULL,
  scope TEXT NOT NULL,
  time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  data TEXT NOT NULL CHECK (json_valid(data)),
  UNIQUE (stream, sequence)
) STRICT;

-- Append-only (plan D1): no event is ever updated, deleted or replaced, whoever
-- writes. INSERT OR REPLACE removes the row it conflicts with without firing a
-- delete trigger, so an insert that would take the ID or the place of an event
-- is refused before SQLite resolves the conflict.
CREATE TRIGGER events_are_never_updated BEFORE UPDATE ON events
BEGIN
  SELECT RAISE(ABORT, 'the journal is append-only: an event is never updated');
END;

CREATE TRIGGER events_are_never_deleted BEFORE DELETE ON events
BEGIN
  SELECT RAISE(ABORT, 'the journal is append-only: an event is never deleted');
END;

CREATE TRIGGER events_are_never_replaced BEFORE INSERT ON events
WHEN EXISTS (SELECT 1 FROM events WHERE id = NEW.id)
  OR EXISTS (SELECT 1 FROM events WHERE stream = NEW.stream AND sequence = NEW.sequence)
BEGIN
  SELECT RAISE(ABORT, 'the journal is append-only: an event is never replaced');
END;

-- How far each consumer has read each stream: the sequence of the last event
-- it acknowledged, which moves forward only (plan D3).
CREATE TABLE cursors (
  consumer TEXT NOT NULL,
  stream TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (consumer, stream)
) STRICT;
