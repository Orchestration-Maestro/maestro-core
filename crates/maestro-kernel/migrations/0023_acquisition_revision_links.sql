-- Native capture/fidelity evidence remains a resolvable immutable S1 relation.
CREATE TABLE acquisition_revision_links (
    revision TEXT NOT NULL REFERENCES revisions(id),
    scope TEXT NOT NULL,
    capture TEXT NOT NULL REFERENCES acquisition_evidence(id),
    fidelity TEXT NOT NULL REFERENCES acquisition_evidence(id),
    inventory TEXT NOT NULL REFERENCES artifacts(digest),
    PRIMARY KEY (revision, capture)
) STRICT;
CREATE INDEX acquisition_revision_links_capture ON acquisition_revision_links(capture);
CREATE TRIGGER acquisition_revision_links_never_change
BEFORE UPDATE ON acquisition_revision_links
BEGIN
    SELECT RAISE(ABORT, 'revision link is immutable');
END;
CREATE TRIGGER acquisition_revision_links_never_delete
BEFORE DELETE ON acquisition_revision_links
BEGIN
    SELECT RAISE(ABORT, 'revision link is immutable');
END;
