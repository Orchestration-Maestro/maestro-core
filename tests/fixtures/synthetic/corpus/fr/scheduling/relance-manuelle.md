# Relancer une tâche à la main

Quand une tâche a épuisé ses tentatives automatiques, elle reste à l'état
`failed` jusqu'à ce qu'une personne la relance.

## Relancer depuis le début

```sh
platformctl job rerun invoice-export --reason "fichier source corrigé"
```

La relance utilise la définition actuelle de la tâche, et non la définition
de l'exécution en échec : une correction de la définition est donc prise en
compte.

## Reprendre à une étape

Pour une tâche en plusieurs étapes, `--from-step` reprend à l'étape indiquée
sans rejouer les précédentes :

```sh
platformctl job rerun invoice-export --from-step upload --reason "envoi interrompu"
```

Les étapes précédentes doivent avoir réussi pendant l'exécution en échec ;
sinon, la commande refuse la relance avec l'erreur `JOB-409`.

## Délai de relance

Une exécution en échec peut être relancée pendant `rerun_window_hours`,
72 heures par défaut. Au-delà, ses fichiers temporaires sont supprimés et
seule une nouvelle exécution complète reste possible.

## Qui peut relancer

Seuls les membres de l'équipe propriétaire de la tâche et les ingénieurs
d'astreinte peuvent la relancer. Chaque relance est tracée dans le journal
d'audit, avec le nom de la personne et le motif donné par `--reason`.
