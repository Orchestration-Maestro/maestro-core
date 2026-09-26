# Running schema migrations

Schema changes are applied by migrations: numbered SQL files that run in
order and are recorded in the table `schema_history`.

## Naming migration files

Each file is named `V<number>__<description>.sql`, for example
`V0042__add_orders_status_index.sql`. A number is never reused, and a
migration that has been applied anywhere is never edited: write a new one
instead.

## Applying migrations

```sh
platformctl migrate status --database orders
platformctl migrate up --database orders
```

`migrate up` applies every pending migration in one run, each in its own
transaction, and stops at the first one that fails.

## Locks and long migrations

Before it starts, `migrate up` takes an advisory lock, so that two runs never
apply migrations at the same time. A second run waits for the lock up to
`lock_timeout_seconds`, 60 by default, then fails with `MIG-423`. A migration
that rewrites a large table should be split into small batches.

## Rolling back

Migrations are never rolled back automatically. To undo one, write a new
migration that reverses it. `migrate undo` exists for development databases
only, and refuses to run where the configuration sets
`environment: production`.

## Errors

| Code | Meaning |
| --- | --- |
| `MIG-409` | The file of an applied migration has changed: its checksum no longer matches `schema_history` |
| `MIG-423` | The migration lock could not be taken within `lock_timeout_seconds` |
| `MIG-500` | A migration failed; the database's error follows in the log |
