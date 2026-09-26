# Limites de débit

Pour protéger la plateforme, le nombre de requêtes de chaque client est
limité par minute.

## Quotas

| Offre | Requêtes par minute | Rafale admise |
| --- | --- | --- |
| Standard | 600 | 100 |
| Étendue | 3 000 | 500 |
| Interne | 20 000 | 2 000 |

Le quota s'applique à chaque jeton d'accès, et non à chaque adresse IP.

## En-têtes de réponse

Chaque réponse indique au client où il en est :

- `X-RateLimit-Limit` : son quota par minute ;
- `X-RateLimit-Remaining` : les requêtes qui lui restent dans la minute
  actuelle ;
- `X-RateLimit-Reset` : le nombre de secondes avant la remise à zéro.

## Dépassement

Au-delà du quota, l'API répond `429` avec le code `rate_limited` et l'en-tête
`Retry-After`. Un client qui continue d'envoyer des requêtes plus de
10 minutes après un premier `429` est bloqué pendant une heure.
