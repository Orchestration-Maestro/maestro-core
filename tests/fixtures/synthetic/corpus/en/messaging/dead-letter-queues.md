# Dead-letter queues

A message that cannot be processed is moved to a dead-letter queue instead of
being retried forever. Every queue `name` gets its dead-letter queue
`name.dlq` when it is created.

## When a message is dead-lettered

A message moves to the dead-letter queue when:

- it has been delivered `max_redeliveries` times without being acknowledged,
  5 by default;
- it has stayed in its queue longer than its time to live, `message_ttl`;
- a consumer rejects it with `requeue=false`.

The broker adds the headers `x-death-reason` and `x-first-death-at` to every
message it dead-letters.

## Inspecting dead-lettered messages

```sh
platformctl queue peek --queue billing-events.dlq --limit 20
```

`peek` shows the messages without removing them from the queue.

## Replaying messages

Once the cause is fixed, move the messages back to their original queue:

```sh
platformctl queue replay --from billing-events.dlq --to billing-events --rate 50
```

`--rate` limits the replay to 50 messages per second, so that the consumers
are not flooded.

## Limits and errors

A dead-letter queue holds at most 100,000 messages or 1 GiB. When it is full,
new dead-lettered messages are dropped and the broker logs `MQ-2203`. The
error `MQ-1408` means that a replay was refused because its target queue does
not exist.
