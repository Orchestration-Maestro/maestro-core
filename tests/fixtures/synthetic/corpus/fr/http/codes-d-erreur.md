# Codes d'erreur HTTP

Chaque API de la plateforme signale une erreur par un code de statut HTTP et
un corps JSON. Un client doit se fier au code stable `error_code`, jamais au
texte `message`, destiné aux personnes.

## Corps d'erreur

```json
{
  "error_code": "version_conflict",
  "message": "The resource was changed by another request.",
  "details": [],
  "request_id": "req-7f3a9c"
}
```

Donnez le `request_id` quand vous contactez le support : il identifie la
requête dans tous les journaux.

## Erreurs du client

| Statut | Code d'erreur | Signification |
| --- | --- | --- |
| 400 | `invalid_request` | Le corps n'est pas du JSON valide ou un champ a le mauvais type ; `details` liste les champs |
| 401 | `invalid_token` | Le jeton d'accès est absent, mal formé ou expiré |
| 403 | `forbidden` | Le jeton est valable, mais il n'a pas la portée que l'opération exige |
| 404 | `not_found` | La ressource n'existe pas, ou l'appelant n'a pas le droit de savoir qu'elle existe |
| 409 | `version_conflict` | La version donnée par `If-Match` n'est pas la version actuelle de la ressource |
| 422 | `validation_failed` | La requête est bien formée, mais elle enfreint une règle métier |

Une erreur du client ne se réessaie jamais telle quelle : il faut commencer
par modifier la requête.

## Limitation de débit

Un client qui envoie trop de requêtes reçoit `429 Too Many Requests` avec le
code d'erreur `rate_limited`. L'en-tête `Retry-After` donne le nombre de
secondes à attendre avant la requête suivante.

## Erreurs du serveur

| Statut | Code d'erreur | Signification |
| --- | --- | --- |
| 500 | `internal_error` | Une défaillance inattendue, journalisée avec le `request_id` |
| 502 | `bad_gateway` | Un service en amont a renvoyé une réponse incorrecte |
| 503 | `service_unavailable` | Le service est surchargé ou en maintenance |
| 504 | `upstream_timeout` | Un service en amont n'a pas répondu à temps |

## Réessayer sans risque

Les erreurs du serveur se réessaient avec un délai exponentiel, qui commence
à 1 seconde et double jusqu'à 30 secondes. Un `POST` ne se réessaie sans
risque que s'il porte l'en-tête `Idempotency-Key` : le serveur renvoie alors
le premier résultat au lieu d'exécuter l'opération deux fois. Les clés sont
gardées 24 heures.
