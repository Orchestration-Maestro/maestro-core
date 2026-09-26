# HTTP error codes

Every API of the platform reports an error with an HTTP status code and a
JSON body. Clients should branch on the stable `error_code`, never on the
human-readable `message`.

## Error body

```json
{
  "error_code": "version_conflict",
  "message": "The resource was changed by another request.",
  "details": [],
  "request_id": "req-7f3a9c"
}
```

Quote the `request_id` when you contact support: it identifies the request in
every log.

## Client errors

| Status | Error code | Meaning |
| --- | --- | --- |
| 400 | `invalid_request` | The body is not valid JSON or a field has the wrong type; `details` lists the fields |
| 401 | `invalid_token` | The access token is missing, malformed or expired |
| 403 | `forbidden` | The token is valid but lacks the scope that the operation needs |
| 404 | `not_found` | The resource does not exist, or the caller may not know that it does |
| 409 | `version_conflict` | The `If-Match` version is not the resource's current version |
| 422 | `validation_failed` | The request is well formed but breaks a business rule |

A client error is never retried as it is: the request must be changed first.

## Rate limiting

A client that sends too many requests receives `429 Too Many Requests` with
the error code `rate_limited`. The `Retry-After` header gives the number of
seconds to wait before the next request.

## Server errors

| Status | Error code | Meaning |
| --- | --- | --- |
| 500 | `internal_error` | An unexpected failure, logged with the `request_id` |
| 502 | `bad_gateway` | An upstream service answered with an invalid response |
| 503 | `service_unavailable` | The service is overloaded or in maintenance |
| 504 | `upstream_timeout` | An upstream service did not answer in time |

## Retrying safely

Retry server errors with exponential backoff, starting at 1 second and
doubling up to 30 seconds. A `POST` is safe to retry only when it carries an
`Idempotency-Key` header: the server then returns the first result instead of
performing the operation twice. Keys are remembered for 24 hours.
