# Log rotation

Services write their logs under `/var/log/platform/<service>/`, and rotation
keeps those directories from filling the disk. It is configured in
`/etc/platform/rotation.yaml`.

## Rotation by size

A log file is rotated when it reaches `max_size`, 100 MiB by default. The
current file is renamed with a numeric suffix, `app.log.1`, and the service
goes on writing to a new `app.log`.

## Example

Rotate the gateway's access log at 50 MiB and keep ten files:

```yaml
- path: /var/log/platform/gateway/access.log
  max_size: 50MiB
  keep: 10
```

## Rotation by time

Set `interval` to `daily` or `weekly` to rotate on a schedule instead. Daily
rotation happens at midnight UTC, and the rotated file carries the date, as
in `app.log.2026-03-14`.

## Example

Rotate the scheduler's log every day and keep two weeks of files:

```yaml
- path: /var/log/platform/scheduler/scheduler.log
  interval: daily
  keep: 14
```

## Compression and retention

Rotated files are compressed with gzip from their second rotation on, so the
most recent rotated file stays readable with plain tools. When more than
`keep` rotated files exist, the oldest one is deleted. A rotation that fails
is logged with the code `LOG-042` and tried again at the next check, one
minute later.
