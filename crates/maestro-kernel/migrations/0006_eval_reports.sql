-- The evaluation reports (plan D13; FR-S1-009): one row for each run of a
-- suite over a generation of a collection, whose report,
-- `maestro-eval-report/1`, is the artifact the row names by its digest and
-- pins. A report is recorded for a generation of its own collection, and
-- keeps that generation's id once the generation's row is gone: a
-- measurement outlives the generation it measured.
CREATE TABLE eval_reports (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  generation_id INTEGER NOT NULL,
  suite TEXT NOT NULL,
  digest TEXT NOT NULL,
  recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX eval_reports_by_collection ON eval_reports (collection_id);

-- A report is a measurement: it is never updated, replaced nor deleted,
-- whoever writes. INSERT OR REPLACE removes the row it conflicts with without
-- firing a delete trigger, so an insert that would take the id of a report is
-- refused before SQLite resolves the conflict.
CREATE TRIGGER eval_reports_are_never_updated BEFORE UPDATE ON eval_reports
BEGIN
  SELECT RAISE(ABORT, 'an evaluation report is never updated: it is a measurement');
END;

CREATE TRIGGER eval_reports_are_never_deleted BEFORE DELETE ON eval_reports
BEGIN
  SELECT RAISE(ABORT, 'an evaluation report is never deleted: it is a measurement');
END;

CREATE TRIGGER eval_reports_are_never_replaced BEFORE INSERT ON eval_reports
WHEN EXISTS (SELECT 1 FROM eval_reports WHERE id = NEW.id)
BEGIN
  SELECT RAISE(ABORT, 'an evaluation report is never replaced: it is a measurement');
END;
