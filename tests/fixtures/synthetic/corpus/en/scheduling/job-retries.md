# Retrying scheduled jobs

When a scheduled job fails, the scheduler can run it again on its own. This
page explains when it does, how long it waits and when it gives up.

## Retry policy

A job's retries are set in the `retryPolicy` block of its definition:

```yaml
job: invoice-export
schedule: "0 2 * * *"
retryPolicy:
  maxAttempts: 4
  initialDelayMs: 30000
  backoffMultiplier: 2
  retryOn: [timeout, exit-code-75]
```

`maxAttempts` counts the first run, so `maxAttempts: 4` means one run and up
to three retries. A job without a `retryPolicy` is not retried.

## Backoff between attempts

The delay before a retry is `initialDelayMs`, multiplied by
`backoffMultiplier` once for every earlier retry, and never more than
15 minutes:

| Attempt | Delay before it |
| --- | --- |
| 1 | None |
| 2 | 30 s |
| 3 | 60 s |
| 4 | 120 s |

A random jitter of up to 10 % is added, so that jobs failing together do not
retry together.

## Which failures are retried

Only the failures listed in `retryOn` are retried: `timeout` when the job
runs longer than its `timeoutSeconds`, and `exit-code-N` for the exit code N.
A job killed by an operator is never retried.

## Idempotent jobs

A retried job starts again from the beginning. Write jobs so that running
them twice has the same effect as running them once: use upserts rather than
inserts, and write output files under a temporary name before renaming them.

## After the last attempt

When the last attempt fails, the job moves to the `failed` state, the on-call
engineer is paged, and the job waits for someone to run it again by hand.
