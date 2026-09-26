# Renewing TLS certificates

Every public endpoint of the platform serves a certificate issued by the
internal certificate authority. This page describes how those certificates
are renewed in release 4.2.

## Validity and renewal window

Certificates are issued for 90 days. Renewal starts 30 days before expiry, as
set by `renew_before_days`, and is retried every 12 hours until it succeeds.

## Automatic renewal

Automatic renewal is on by default. Each endpoint controls it with the
`autoRenew` field of its definition:

```json
{
  "endpoint": "api-public",
  "tls": { "autoRenew": true, "keyType": "ecdsa-p256" }
}
```

New keys are ECDSA P-256 unless `keyType` says otherwise; RSA 2048 remains
available as `rsa-2048`.

## Renewing by hand

```sh
platformctl cert renew --endpoint api-public
```

Add `--dry-run` to check that the request would be accepted without issuing
anything.

## Where certificates are kept

Each endpoint's certificate chain and private key are written to
`/etc/platform/tls/<endpoint>/` as `fullchain.pem` and `privkey.pem`. The key
file is readable by the endpoint's service account only (mode `0600`). The
previous pair is kept as `fullchain.pem.previous` and `privkey.pem.previous`
until the next renewal, so that a faulty certificate can be rolled back:

```sh
platformctl cert rollback --endpoint api-public
```

## Checking a certificate

`platformctl cert show --endpoint api-public` prints the certificate's
subject, issuer, serial number and key type, and the dates between which it
is valid. Add `--chain` to print the intermediate certificates as well.

## Expiry alerts

An alert is raised 14 days before expiry if the certificate has not been
renewed, and again every day after that. The error `TLS-017` in the renewal
log means that the authority refused the request because the endpoint's
domain is no longer validated.
