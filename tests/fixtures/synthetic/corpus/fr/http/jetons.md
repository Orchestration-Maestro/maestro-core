# Jetons d'accès à l'API

Chaque appel à l'API porte un jeton d'accès dans l'en-tête `Authorization`,
sous la forme `Bearer <jeton>`.

## Obtenir un jeton

Un service obtient un jeton auprès du point d'accès `/oauth/token` avec ses
identifiants client. La réponse donne la durée de vie du jeton en secondes
dans `expires_in` : 3 600, soit une heure.

## Portées

Un jeton ne donne accès qu'aux opérations de ses portées, par exemple
`invoices:read` ou `invoices:write`. Un appel hors des portées du jeton est
refusé avec `403 forbidden`, alors qu'un jeton absent ou expiré donne
`401 invalid_token`.

## Renouvellement

Un client doit demander un nouveau jeton avant que le précédent expire, par
exemple quand il lui reste moins de 5 minutes. Un jeton expiré n'est jamais
accepté : il n'y a pas de délai de grâce.

## Rotation des identifiants client

Les identifiants client, `client_id` et `client_secret`, sont renouvelés tous
les 180 jours. Pendant 7 jours, l'ancien et le nouveau secret sont acceptés
tous les deux, le temps de déployer le nouveau.
