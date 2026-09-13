# ADR 0001 : Décodeurs eD2K en Lecture Seule sans Encodeur ni Session Connectée

## Statut
Accepté

## Date
2026-09-13

## Contexte
L'intégration du protocole historique eDonkey2000 / eMule représente un risque architectural et de sécurité majeur si elle est abordée par l'écriture simultanée de clients réseau actifs, de machines à états de session et d'encodeurs. Les spécifications historiques sont dispersées dans des codebases C++ vieilles de plus de vingt ans (eMule MFC, aMule).

## Décision
Pour la Phase 5 :
1. Seuls des décodeurs en lecture seule (`Ed2kDecoder`) et des structures de données passives d'observation (`Ed2kHello`, `Ed2kHelloAnswer`, `Ed2kFileStatus`) sont implémentés dans `mule-protocol::ed2k`.
2. Aucun encodeur de paquets eD2K actif n'est produit.
3. Aucune machine à états de session active (gestionnaire d'envoi/réception, timer de ping, négociation d'upload/download) n'est associée à ces structures.
4. Toute validation s'effectue exclusivement contre un corpus de fixtures binaires statiques et reproductibles.

## Conséquences
- **Positives** : Risque d'erreur ou d'effet de bord réseau nul, surface d'attaque minimale, auditabilité totale des parseurs avant toute tentative d'émission.
- **Négatives** : Le client ne transfère pas encore de données avec des pairs eMule réels (ce qui est l'objectif voulu de cette phase passive).
