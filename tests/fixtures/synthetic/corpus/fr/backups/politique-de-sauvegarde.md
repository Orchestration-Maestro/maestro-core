# Politique de sauvegarde

Cette politique s'applique à toutes les bases de données et à tous les
stockages de fichiers de production que la plateforme exploite. La
préproduction ne la suit que si une équipe le demande par écrit.

## Types de sauvegarde et calendrier

| Type | Exécution | Contenu copié |
| --- | --- | --- |
| Complète | Le dimanche à 01:00 UTC | Toutes les bases et tous les stockages de fichiers |
| Incrémentale | Du lundi au samedi à 01:00 UTC | Les changements depuis la sauvegarde précédente, quel que soit son type |
| Journal de transactions | Toutes les 15 minutes | Le journal d'écriture anticipée de chaque base |

L'ordonnanceur exécute la sauvegarde complète sous le nom de tâche
`nightly-full-backup` et la sauvegarde incrémentale sous le nom
`nightly-incremental-backup`. Une sauvegarde qui démarre avec plus de
2 heures de retard est déclarée manquée, même si elle se termine plus tard.

## Conservation

Les sauvegardes sont conservées pendant le nombre de jours fixé par
`retention_days` dans `backup.yaml`, 35 par défaut. En plus, la première
sauvegarde complète de chaque mois est gardée 13 mois (`keep_monthly: 13`)
et la première de chaque année 7 ans (`keep_yearly: 7`).

```yaml
backup:
  retention_days: 35
  keep_monthly: 13
  keep_yearly: 7
```

## Chiffrement

Chaque sauvegarde est chiffrée avant de quitter son hôte, en AES-256 en mode
GCM. La clé de données est elle-même chiffrée par la clé que désigne
`encryption_key_id`. Cette clé change tous les 90 jours ; les anciennes
sauvegardes restent lisibles, car chacune garde la version de la clé qui a
servi à l'écrire.

## Copies hors site

Un double de chaque sauvegarde complète est envoyé dans une autre région en
moins de 6 heures. Ce double est immuable pendant toute sa durée de
conservation : personne, administrateurs compris, ne peut le supprimer plus
tôt.

## Objectifs de reprise

| Objectif | Cible |
| --- | --- |
| Perte de données maximale admissible (RPO) | 15 minutes |
| Durée maximale d'interruption admissible (RTO) | 4 heures pour une base |
