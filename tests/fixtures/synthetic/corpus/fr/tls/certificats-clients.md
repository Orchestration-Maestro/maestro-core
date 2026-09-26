# Certificats clients et TLS mutuel

Les services internes s'authentifient entre eux par TLS mutuel : chaque
client présente un certificat, et le serveur vérifie qu'il est signé par
l'autorité de certification interne.

## Émission d'un certificat client

```sh
platformctl cert issue --client billing-worker --ttl 72h
```

Un certificat client est valable 72 heures au plus. Son champ CN porte
l'identifiant du service, par exemple `billing-worker`, et c'est ce nom que
les serveurs utilisent dans leurs règles d'accès.

## Renouvellement automatique

L'agent installé sur chaque hôte renouvelle le certificat quand les deux
tiers de sa durée de vie sont écoulés, soit après 48 heures pour un
certificat de 72 heures. Le service n'a pas besoin de redémarrer : l'agent
écrit les nouveaux fichiers, et le service les relit à la connexion suivante.

## Révocation

Un certificat compromis est révoqué avec
`platformctl cert revoke --serial <numéro>`. La liste de révocation est
publiée toutes les 5 minutes, si bien qu'un serveur refuse un certificat
révoqué au plus tard 5 minutes après la révocation.

## Erreurs fréquentes

| Code | Signification |
| --- | --- |
| `TLS-031` | Le certificat client a expiré |
| `TLS-032` | Le certificat n'est pas signé par l'autorité interne |
| `TLS-040` | Le CN du certificat ne figure pas dans les règles d'accès du serveur |
