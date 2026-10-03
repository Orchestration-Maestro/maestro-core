-- Source-backed review pins; claims, frozen sets and attachments are unchanged.
CREATE TABLE graph_resolutions (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 64),
    previous_id TEXT REFERENCES graph_resolutions(id),
    reviewer TEXT NOT NULL CHECK (length(reviewer) > 0),
    body TEXT NOT NULL CHECK (json_valid(body)),
    recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT, WITHOUT ROWID;

CREATE TRIGGER graph_resolutions_immutable_insert BEFORE INSERT ON graph_resolutions
WHEN EXISTS (SELECT 1 FROM graph_resolutions WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'resolution snapshots cannot be replaced'); END;
CREATE TRIGGER graph_resolutions_immutable_update BEFORE UPDATE ON graph_resolutions
BEGIN SELECT RAISE(ABORT, 'resolution snapshots are immutable'); END;
CREATE TRIGGER graph_resolutions_immutable_delete BEFORE DELETE ON graph_resolutions
BEGIN SELECT RAISE(ABORT, 'resolution snapshots are immutable'); END;
