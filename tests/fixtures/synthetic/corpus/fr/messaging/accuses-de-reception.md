# Accusés de réception

Un consommateur signale au courtier qu'il a traité un message en lui
envoyant un accusé de réception. Tant que cet accusé n'est pas reçu, le
message peut être livré de nouveau.

## Modes d'accusé

| Mode | Fonctionnement | Risque |
| --- | --- | --- |
| `auto` | Le message est confirmé dès sa livraison | Le message est perdu si le consommateur tombe pendant le traitement |
| `manual` | Le consommateur acquitte le message après le traitement | Le message est traité deux fois si le consommateur tombe avant de confirmer |
| `batch` | Le consommateur acquitte un lot de messages en une fois | Tout le lot est livré de nouveau en cas d'échec |

Le mode `manual` est recommandé : il garantit que chaque message est livré au
moins une fois.

## Délai de visibilité

Un message livré devient invisible pour les autres consommateurs pendant
`visibilityTimeout`, 30 secondes par défaut. S'il n'est pas confirmé avant la
fin de ce délai, il redevient visible et sera livré à un autre consommateur.

## Préchargement

`prefetchCount` règle le nombre de messages qu'un consommateur peut recevoir
sans les avoir encore confirmés, 10 par défaut. Une valeur trop grande
retient des messages chez un consommateur lent ; la valeur 1 donne la
répartition la plus équitable.
