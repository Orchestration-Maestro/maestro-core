# Glossary

The terms this handbook uses, grouped by subject.

## Backup and recovery

- **RPO**, recovery point objective: how much recent data a restore may lose,
  measured in time.
- **RTO**, recovery time objective: how long a restore may take.
- **Snapshot**: a consistent copy of a database at one moment.

## Scheduling

- **Job**: a unit of work that the scheduler runs on a schedule or on demand.
- **Backoff**: the growing delay between two attempts of a failed job.
- **Idempotent**: said of an operation that has the same effect whether it
  runs once or several times.

## Messaging

- **Consumer**: a process that receives messages from a queue.
- **Dead-letter queue**: where a queue puts the messages it cannot deliver.
- **Acknowledgement**: a consumer's confirmation that it has processed a
  message.

## Security

- **Certificate authority**: the service that signs certificates.
- **Mutual TLS**: TLS in which the client presents a certificate too.
- **Scope**: a permission that an access token carries.
