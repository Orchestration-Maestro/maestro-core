-- Checkpoints and accepted snapshots are separate, immutable evidence.
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
