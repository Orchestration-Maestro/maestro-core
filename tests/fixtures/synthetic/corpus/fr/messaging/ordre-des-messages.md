# Ordre des messages et partitions

Le courtier de messages ne garantit l'ordre des messages qu'à l'intérieur
d'une partition. Cette page explique comment choisir la clé de partition qui
donne l'ordre dont une application a besoin.

## Ordre garanti

Les messages qui portent la même clé de partition, `partition_key`, sont
toujours écrits dans la même partition et lus dans leur ordre d'écriture.
Deux messages de clés différentes peuvent être lus dans n'importe quel
ordre.

## Choisir la clé de partition

Prendre comme clé l'identifiant de l'entité dont les événements doivent
rester ordonnés, par exemple `customer_id` pour les événements d'un client.
Une clé qui prend peu de valeurs différentes concentre la charge sur
quelques partitions.

## Nombre de partitions

Une file est créée avec 12 partitions par défaut (`partitions: 12`). Le
nombre de partitions peut être augmenté, jamais diminué, et une augmentation
déplace certaines clés vers une autre partition : l'ordre n'est plus garanti
pendant la transition.

## Groupes de consommateurs

Dans un groupe de consommateurs, chaque partition est lue par un seul
consommateur à la fois. Un groupe de plus de 12 consommateurs sur une file de
12 partitions laisse donc des consommateurs inactifs.
