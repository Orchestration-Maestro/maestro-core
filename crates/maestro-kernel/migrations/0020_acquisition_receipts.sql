-- Opaque access-checked references, not digest capabilities in progress events.
CREATE TABLE acquisition_evidence (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL,
    artifact TEXT NOT NULL REFERENCES artifacts(digest)
) STRICT;
CREATE TABLE acquisition_evidence_links (
    parent TEXT NOT NULL REFERENCES acquisition_evidence(id),
    child TEXT NOT NULL REFERENCES acquisition_evidence(id),
    PRIMARY KEY (parent, child)
) STRICT;
CREATE TABLE acquisition_receipts (
    attempt TEXT PRIMARY KEY NOT NULL,
    run TEXT NOT NULL,
    scope TEXT NOT NULL,
    initial TEXT NOT NULL REFERENCES acquisition_evidence(id),
    terminal TEXT REFERENCES acquisition_evidence(id)
) STRICT;
CREATE INDEX acquisition_receipts_run ON acquisition_receipts(run, attempt);
