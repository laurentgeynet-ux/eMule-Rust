# ADR 0002 : Interdiction Absolue de Connectivité Réseau eD2K Externe

## Statut
Remplacé par [ADR 0004](0004-public-ed2k-kad-production.md) (phase de développement terminée — passage en production)

## Date
2026-09-13

## Contexte
Le réseau public eD2K comporte des serveurs publics hétérogènes, des pairs non authentifiés et des vecteurs d'attaque potentiels (trames malformées, bombes de décompression zlib, inondations de requêtes). Une exposition prématurée d'un nouveau client Rust sans validation approfondie des parseurs et des limites de ressources exposerait le projet à des failles graves.

## Décision
1. Le crate `mule-protocol::ed2k` a l'interdiction formelle de créer des structures de sockets réseau (`TcpStream`, `UdpSocket`), des types d'adresses réseau actives (`SocketAddr`) ou des tâches d'écoute/connexion asynchrones.
2. Le binaire `mule-daemon` et l'interface Flutter ne doivent comporter aucun drapeau, option ou route permettant une connexion réseau à un serveur ou pair public externe.
3. Toute la suite de tests eD2K doit être exécutable hors-ligne sans accès Internet.

## Conséquences
- **Positives** : Environnement de test 100 % déterministe, zéro fuite d'informations, sécurité absolue pour l'environnement de développement.
- **Négatives** : Aucune interaction directe avec les pairs du réseau public à ce stade.

## Historique
Cette interdiction couvrait la phase de construction de l'application. Le point 1 (pureté de `mule-protocol` — zéro I/O réseau dans la crate) **demeure en vigueur** : c'est une propriété architecturale permanente, indépendante du statut réseau du daemon. Les points 2 et 3 sont amendés par l'ADR 0004 qui autorise la connectivité publique en production, sous profil explicite.
