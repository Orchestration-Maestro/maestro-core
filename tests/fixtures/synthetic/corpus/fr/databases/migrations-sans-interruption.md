# Migrations sans interruption de service

Une migration qui verrouille une grande table peut bloquer l'application
pendant des minutes. Cette page explique comment modifier un schéma pendant
que le service reste disponible.

## Étendre puis contracter

Une modification incompatible se fait en trois déploiements :

1. étendre : ajouter la nouvelle colonne ou la nouvelle table, sans rien
   supprimer ;
2. migrer : déployer le code qui écrit dans l'ancienne et dans la nouvelle
   structure, puis recopier les données déjà présentes ;
3. contracter : quand plus rien ne lit l'ancienne structure, la supprimer par
   une nouvelle migration.

## Recopier les données par lots

La recopie se fait par lots de `batch_size` lignes, 1 000 par défaut, avec
une pause de `batch_pause_ms` millisecondes entre deux lots pour laisser
passer les requêtes habituelles :

```sh
platformctl migrate backfill --database orders --task copy-status --batch-size 1000
```

## Créer un index sans bloquer

Sur une grande table, un index se crée avec l'option `concurrently`, dans un
fichier de migration qui commence par la ligne `-- transactional: false`,
car une création concurrente ne peut pas s'exécuter dans une transaction.

## Délai de verrouillage

Chaque migration règle `lock_timeout` à 5 secondes : si elle n'obtient pas
son verrou à temps, elle échoue au lieu de faire attendre les requêtes de
l'application, et on peut la relancer plus tard.
