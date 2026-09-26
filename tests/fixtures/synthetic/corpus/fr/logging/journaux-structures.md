# Journaux structurés

Les services écrivent leurs journaux en JSON, une ligne par événement, pour
qu'on puisse les filtrer et les agréger sans expressions régulières.

## Champs obligatoires

| Champ | Contenu |
| --- | --- |
| `timestamp` | L'heure de l'événement, en UTC, au format RFC 3339 |
| `level` | Le niveau : `debug`, `info`, `warn` ou `error` |
| `service` | Le nom du service |
| `message` | Une phrase courte et stable, sans valeur variable |
| `trace_id` | L'identifiant de la trace, qui rattache les journaux d'une même requête |

## Exemple

```json
{"timestamp":"2026-03-14T09:41:07Z","level":"warn","service":"billing-worker","message":"invoice export retried","trace_id":"4bf92f3577b34da6","attempt":2}
```

Les valeurs variables vont dans des champs à part, comme `attempt`, et
jamais dans `message`.

## Niveaux

En production, le niveau minimal est `info`. Le niveau `debug` peut être
activé pour un service pendant une heure au plus :

```sh
platformctl log level --service billing-worker --level debug --for 1h
```

## Données personnelles

Aucune adresse électronique, aucun numéro de téléphone et aucun jeton d'accès
ne doit apparaître dans un journal. Le collecteur masque les motifs qu'il
reconnaît en les changeant en `[masqué]`, et compte chaque masquage dans la
métrique `log_redactions_total`.
