# ADR 0003 : Séparation Étanche entre le Protocole de Laboratoire et le Protocole Historique eD2K

## Statut
Accepté

## Date
2026-09-13

## Contexte
Le projet a développé en Phase 4 un banc d'essai complet de transfert de fichiers en boucle locale (`mule-protocol::lab`, magic `0xEE`, `LabMessage`) pour prouver la continuité entre framing, planification, stockage temporaire `.part`, calcul de hash eD2K et intégrité SQLite.

Il serait dangereux et confus de tenter de mélanger ou d'étendre directement ce protocole de laboratoire pour absorber les particularités du protocole eD2K historique (`0xE3`, `0xC5`, `0xD4`).

## Décision
1. `mule-protocol::lab` et `mule-protocol::ed2k` sont maintenus dans deux répertoires de code complètement séparés.
2. Aucun type de message n'est partagé entre `lab` et `ed2k`. Les seuls éléments communs sont les primitives de streaming de bas niveau (`RawPacket`, `PacketCodec`, constantes de longueur d'en-tête).
3. Les limites de trame (taille de paquet, taille de chaîne, nombre de tags) sont indépendantes entre les deux modules.

## Conséquences
- **Positives** : Le laboratoire reste une suite de tests robuste et isolée protégée par la feature `lab-loopback`, sans régression ni interférence avec l'archéologie eD2K.
- **Négatives** : Duplication légère de structures de messages similaires (ex: `Hello` vs `Ed2kHello`), mais assurant un découplage total et une indépendance sémantique.
