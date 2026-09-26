# Backup policy

This policy applies to every production database and file store that the
platform operates. Staging environments follow it only when a team asks for
it in writing.

## Backup types and schedule

| Type | When it runs | What it copies |
| --- | --- | --- |
| Full | Sunday at 01:00 UTC | Every database and file store |
| Incremental | Monday to Saturday at 01:00 UTC | The changes since the previous backup, whatever its type |
| Transaction log | Every 15 minutes | The write-ahead log of each database |

The scheduler runs the full backup as the job `nightly-full-backup` and the
incremental one as `nightly-incremental-backup`. A backup that starts more
than 2 hours late is reported as missed, even if it completes later.

## Retention

Backups are kept for the number of days set by `retention_days` in
`backup.yaml`, 35 by default. In addition, the first full backup of each
month is kept for 13 months (`keep_monthly: 13`) and the first of each year
for 7 years (`keep_yearly: 7`).

```yaml
backup:
  retention_days: 35
  keep_monthly: 13
  keep_yearly: 7
```

## Encryption

Every backup is encrypted before it leaves its host, with AES-256 in GCM
mode. The data key is itself encrypted with the key named by
`encryption_key_id`. That key is rotated every 90 days; older backups remain
readable because each one records the key version it was written with.

## Off-site copies

A copy of every full backup is sent to a second region within 6 hours. The
copy is immutable for its whole retention period: nobody, administrators
included, can delete it early.

## Recovery objectives

| Objective | Target |
| --- | --- |
| Recovery point objective (RPO) | 15 minutes |
| Recovery time objective (RTO) | 4 hours for one database |
