# Surveiller les tâches planifiées

L'ordonnanceur publie l'état de chaque tâche et envoie une alerte quand une
tâche prend du retard ou reste bloquée.

## États d'une tâche

| État | Signification |
| --- | --- |
| `scheduled` | La tâche attend son heure de démarrage |
| `running` | La tâche s'exécute |
| `succeeded` | La dernière exécution a réussi |
| `failed` | La dernière tentative a échoué et aucune relance n'est prévue |
| `skipped` | L'exécution a été sautée, car la précédente n'était pas terminée |

## Tâche en retard

### Symptôme

Une tâche est déclarée en retard quand elle n'a pas démarré
`late_after_minutes` minutes après son heure prévue, 10 minutes par défaut.
L'alerte `JOB-LATE` est alors envoyée à l'équipe propriétaire de la tâche.

### Action

Vérifier avec `platformctl job workers` que l'ordonnanceur a des exécuteurs
libres, puis que l'exécution précédente de la tâche est bien terminée.

## Tâche bloquée

### Symptôme

Une tâche est bloquée quand elle s'exécute depuis plus de deux fois sa durée
médiane sur ses 20 dernières exécutions. L'alerte `JOB-STUCK` est alors
envoyée.

### Action

Lire les dernières lignes du journal de la tâche avec
`platformctl job logs --tail 100 <tâche>`. Si la tâche ne progresse plus,
l'arrêter avec `platformctl job kill <tâche>` : elle passe à l'état `failed`
et n'est pas relancée automatiquement.
