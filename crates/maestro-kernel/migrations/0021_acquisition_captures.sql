-- Prepared captures survive an interruption before stage acknowledgment.
-- One immutable linkage per distinct request/authorization/representation item.
CREATE TABLE acquisition_capture_links (
    item TEXT NOT NULL REFERENCES acquisition_frontier(id),
    generation INTEGER NOT NULL CHECK (generation >= 0),
    identity TEXT NOT NULL,
    envelope TEXT NOT NULL UNIQUE REFERENCES acquisition_evidence(id),
    body TEXT NOT NULL REFERENCES acquisition_evidence(id),
    PRIMARY KEY (item, generation)
) STRICT;
CREATE TRIGGER acquisition_capture_links_never_change
BEFORE UPDATE ON acquisition_capture_links
BEGIN
    SELECT RAISE(ABORT, 'capture link is immutable');
END;
CREATE TRIGGER acquisition_capture_links_never_delete
BEFORE DELETE ON acquisition_capture_links
BEGIN
    SELECT RAISE(ABORT, 'capture link is immutable');
END;
