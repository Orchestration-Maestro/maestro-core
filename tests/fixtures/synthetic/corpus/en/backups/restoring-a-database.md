# Restoring a database from a backup

Use this runbook when a database must be brought back to an earlier state,
after data loss or a faulty deployment.

## Before you start

- Open an incident and name the one person who runs the restore.
- Stop every writer of the database: scale the services that write to it to
  zero, or put them in maintenance mode.
- Check that the target host has free disk space of at least 1.5 times the
  size of the backup.

## Restore the latest backup

1. List the available snapshots:

   ```sh
   platformctl backup list --database orders
   ```

2. Restore the most recent one into a new instance:

   ```sh
   platformctl backup restore --database orders --snapshot latest --target orders-restore
   ```

3. Switch the services to `orders-restore` only after the checks below pass.

## Restore to a point in time

To undo a faulty change, restore to the minute before it with
`--point-in-time`, given in UTC:

```sh
platformctl backup restore --database orders --point-in-time 2026-03-14T09:41:00Z --target orders-restore
```

The restore replays the transaction logs on top of the last full or
incremental backup taken before that time, so it can reach back only as far
as the retention period.

## Check the restored data

- Compare the row counts of the largest tables with their last known values.
- Run the application's read-only smoke tests against `orders-restore`.
- Keep the old instance, stopped, for 7 days before deleting it.

## Troubleshooting

| Code | Meaning | What to do |
| --- | --- | --- |
| `BKP-104` | Snapshot not found | Run `platformctl backup list` again: the snapshot may have expired |
| `BKP-221` | Checksum mismatch in a backup file | Restore the previous snapshot and report the corrupt one |
| `BKP-307` | Not enough disk space on the target | Free some space or choose a larger target host |
