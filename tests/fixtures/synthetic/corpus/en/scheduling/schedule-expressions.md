# Schedule expressions

The `schedule` field of a job takes an expression of five fields, read in the
job's time zone.

## Fields

| Position | Field | Allowed values |
| --- | --- | --- |
| 1 | Minute | 0-59 |
| 2 | Hour | 0-23 |
| 3 | Day of month | 1-31 |
| 4 | Month | 1-12 |
| 5 | Day of week | 0-6, where 0 is Sunday |

Each field accepts `*` for every value, lists such as `1,15`, ranges such as
`1-5` and steps such as `*/10`.

## Examples

| Expression | Runs |
| --- | --- |
| `0 2 * * *` | Every day at 02:00 |
| `*/10 * * * *` | Every 10 minutes |
| `30 6 * * 1-5` | At 06:30 on weekdays |
| `0 0 1 * *` | At midnight on the first day of each month |

## Time zones

A job runs in UTC unless its definition sets `timeZone`, for example
`timeZone: Europe/Paris`. When the clocks move forward, a run scheduled in
the skipped hour starts at the first minute after the change; when they move
back, a run in the repeated hour starts once, at its first occurrence.

## Common mistakes

- Setting both the day of month and the day of week: the job then runs when
  either one matches, not when both do.
- Using `0 */2 * * *` to mean every two hours from now: steps count from the
  start of the range, so it runs at even hours only.
- Forgetting that a run is skipped while the previous run of the same job is
  still going, unless the job sets `allowOverlap: true`.
