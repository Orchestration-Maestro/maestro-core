# Vérifier les sauvegardes

Une sauvegarde qui n'a jamais été restaurée n'est pas une sauvegarde fiable.
Cette page décrit les contrôles automatiques et le test de restauration
mensuel.

## Contrôles automatiques

Après chaque sauvegarde, l'agent de sauvegarde calcule l'empreinte SHA-256
de chaque fichier et la compare à l'empreinte enregistrée à l'écriture.
Chaque nuit, `platformctl backup verify --sample 5` restaure en plus cinq
tables tirées au hasard dans une instance temporaire, qu'il supprime
ensuite.

## Test de restauration mensuel

Le premier mardi de chaque mois, l'équipe d'exploitation restaure une base
entière à partir de la dernière sauvegarde complète, sur un hôte isolé du
réseau de production. Le test est réussi si :

- la restauration se termine en moins de 4 heures ;
- les contrôles de cohérence de l'application passent ;
- aucun fichier de la sauvegarde n'est signalé comme corrompu.

## Rapport de vérification

Le résultat de chaque test est consigné dans un rapport qui donne la
durée de la restauration, la taille de la base et les anomalies relevées.
Deux tests manqués de suite déclenchent une revue avec le responsable de la
sécurité.

## En cas d'échec

Quand une vérification échoue, la sauvegarde concernée est marquée `suspect`
et `platformctl backup list` ne la propose plus. Une nouvelle sauvegarde
complète est lancée aussitôt, sans attendre le dimanche.
