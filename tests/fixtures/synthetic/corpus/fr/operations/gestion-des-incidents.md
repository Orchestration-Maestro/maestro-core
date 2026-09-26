# Gestion des incidents

Un incident est toute interruption ou dégradation d'un service de
production. Cette page définit les niveaux de gravité et les délais de prise en
charge.

## Niveaux de gravité

| Niveau | Définition | Délai de prise en charge |
| --- | --- | --- |
| `SEV1` | Le service est indisponible pour tous les clients | 15 minutes, jour et nuit |
| `SEV2` | Une fonctionnalité majeure est dégradée ou indisponible pour une partie des clients | 30 minutes, jour et nuit |
| `SEV3` | Une gêne mineure, avec un contournement | Le jour ouvré suivant |

## Rôles

- Le responsable de l'incident coordonne et décide ; il ne corrige pas
  lui-même.
- Le chargé de communication tient les clients au courant toutes les
  30 minutes pour un `SEV1`.
- Les intervenants techniques diagnostiquent et corrigent.

## Clôture

Un incident est clôturé quand le service est rétabli et qu'il a été surveillé
une heure sans nouvelle alerte. Un `SEV1` ou un `SEV2` donne lieu, dans les
cinq jours ouvrés, à une revue qui cherche des causes, pas des coupables.
