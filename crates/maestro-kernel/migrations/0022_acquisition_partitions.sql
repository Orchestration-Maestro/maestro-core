-- Checkpoints and accepted snapshots are separate, immutable evidence.
-- Batch artifacts carry bounded not_enqueued digest/reason records, never URLs.
CREATE TABLE acquisition_partitions (
    id TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL REFERENCES acquisition_sources(source),
    scope TEXT NOT NULL
) STRICT;
CREATE TABLE acquisition_partition_batches (
    partition TEXT NOT NULL REFERENCES acquisition_partitions(id),
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    cursor TEXT NOT NULL,
    evidence TEXT NOT NULL REFERENCES acquisition_evidence(id),
    PRIMARY KEY (partition, sequence),
    UNIQUE (partition, cursor)
) STRICT;
CREATE TABLE acquisition_partition_snapshots (
    partition TEXT PRIMARY KEY NOT NULL REFERENCES acquisition_partitions(id),
    evidence TEXT NOT NULL REFERENCES acquisition_evidence(id)
) STRICT;
CREATE TRIGGER acquisition_partition_batches_never_change
BEFORE UPDATE ON acquisition_partition_batches BEGIN
    SELECT RAISE(ABORT, 'partition checkpoint is immutable');
END;
CREATE TRIGGER acquisition_partition_batches_never_delete
BEFORE DELETE ON acquisition_partition_batches BEGIN
    SELECT RAISE(ABORT, 'partition checkpoint is immutable');
END;
CREATE TRIGGER acquisition_partition_snapshots_never_change
BEFORE UPDATE ON acquisition_partition_snapshots BEGIN
    SELECT RAISE(ABORT, 'accepted partition is immutable');
END;
CREATE TRIGGER acquisition_partition_snapshots_never_delete
BEFORE DELETE ON acquisition_partition_snapshots BEGIN
    SELECT RAISE(ABORT, 'accepted partition is immutable');
END;

-- Distinct request/context identities, retained atomically with each checkpoint.
CREATE TABLE acquisition_partition_items (
    partition TEXT NOT NULL REFERENCES acquisition_partitions(id),
    fetch_identity TEXT NOT NULL,
    authorization_context TEXT NOT NULL,
    representation_profile TEXT NOT NULL,
    PRIMARY KEY (partition, fetch_identity, authorization_context, representation_profile)
) STRICT;
CREATE TRIGGER acquisition_partition_items_never_change
BEFORE UPDATE ON acquisition_partition_items BEGIN
    SELECT RAISE(ABORT, 'partition inventory is immutable');
END;
CREATE TRIGGER acquisition_partition_items_never_delete
BEFORE DELETE ON acquisition_partition_items BEGIN
    SELECT RAISE(ABORT, 'partition inventory is immutable');
END;


-- Derived immutable metadata; Batch artifacts remain the authoritative evidence.
CREATE TABLE acquisition_partition_summaries (
    partition TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    run TEXT NOT NULL,
    kind TEXT NOT NULL,
    window TEXT NOT NULL,
    verification_final INTEGER NOT NULL CHECK (verification_final IN (0, 1)),
    committable INTEGER NOT NULL CHECK (committable IN (0, 1)),
    capture TEXT REFERENCES acquisition_evidence(id),
    PRIMARY KEY (partition, sequence),
    FOREIGN KEY (partition, sequence) REFERENCES acquisition_partition_batches(partition, sequence)
) STRICT;
CREATE INDEX acquisition_partitions_source_history ON acquisition_partitions(scope, source, id);
CREATE TRIGGER acquisition_partition_summaries_never_change
BEFORE UPDATE ON acquisition_partition_summaries BEGIN
    SELECT RAISE(ABORT, 'partition summary is immutable');
END;
CREATE TRIGGER acquisition_partition_summaries_never_delete
BEFORE DELETE ON acquisition_partition_summaries BEGIN
    SELECT RAISE(ABORT, 'partition summary is immutable');
END;

-- First-in-partition-ID-order depth for each effective identity, not a work queue.
CREATE TABLE acquisition_depths (
    source TEXT NOT NULL REFERENCES acquisition_sources(source),
    scope TEXT NOT NULL,
    fetch_identity TEXT NOT NULL,
    authorization_context TEXT NOT NULL,
    representation_profile TEXT NOT NULL,
    partition TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    depth TEXT NOT NULL,
    capture TEXT NOT NULL REFERENCES acquisition_evidence(id),
    PRIMARY KEY (source, scope, authorization_context, representation_profile, fetch_identity),
    FOREIGN KEY (partition, sequence) REFERENCES acquisition_partition_batches(partition, sequence)
) STRICT;
