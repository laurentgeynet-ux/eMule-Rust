# Spécification eD2K - Périmètre & Frontières Passives (Phase 5)

## 1. Objectif de la Phase 5
La Phase 5 d'eMuleRust est dédiée à **l'archéologie logicielle, la spécification formelle et l'implémentation de décodeurs en lecture seule** pour le protocole historique eDonkey2000 (eD2K) et ses extensions eMule.

Cette phase a pour but d'établir une compréhension rigoureuse, versionnée et testée des trames binaires historiques, en se basant exclusivement sur des spécifications documentées, des fixtures binaires figées et des implémentations de référence open-source.

---

## 2. Règles Fondamentales & Interdictions Formelles

### 2.1 Zéro Connexion Réseau Externe (100 % Hors-Ligne)
- **Interdiction absolue de communication réseau** : Le sous-module `mule-protocol::ed2k` ne doit **jamais** ouvrir de socket réseau (`TcpStream`, `UdpSocket`), ni résoudre de nom d'hôte ou d'adresse IP (`SocketAddr`), ni écouter sur une interface réseau locale ou publique.
- **Aucune connexion publique** : Le client ne doit sous aucun prétexte tenter de contacter des serveurs eD2K publics, des pairs sur Internet, ou le réseau distribué Kademlia (Kad).
- **Pas d'émission de paquets eD2K** : Aucun encodeur actif destiné à être envoyé sur un réseau eD2K réel n'est créé lors de cette phase. Seules des structures passives d'observation et des fonctions de décodage sont autorisées.

### 2.2 Découplage Total avec le Banc d'Essai de Laboratoire
- Le protocole de laboratoire `PR_LAB` (`0xEE`) utilisé dans les phases 4.1 à 4.4 reste confiné dans `mule-protocol::lab`.
- Aucun type ou message de `mule-protocol::ed2k` ne dépend du module `lab`.
- Les limites de trames, la gestion des identifiants et les décodeurs eD2K sont complètement autonomes.

### 2.3 Absence d'État de Session Connectée
- Les structures de données eD2K produites par les décodeurs (`Ed2kHello`, `Ed2kHelloAnswer`, `Ed2kFileStatus`) sont de pures structures d'observation passives (`value objects`).
- Aucune machine à états de session, aucun gestionnaire d'envoi ou de réception connecté n'est instancié pour eD2K lors de cette phase.

---

## 3. Stratégie de Validation & Critères d'Acceptation
- **Corpus figé de fixtures** : La conformité des décodeurs est validée exclusivement contre des fichiers binaires versionnés dans `crates/mule-protocol/tests/fixtures/ed2k/`, référencés dans un manifeste avec hachage SHA-256.
- **Sécurité anti-déni de service** : Tout décodage vérifie strictement les plafonds de taille de trame, de longueur de chaîne et de quantité de tags avant toute réservation mémoire.
- **Zéro panique** : Aucun `unwrap()` ou `expect()` n'est toléré dans le chemin de décodage.
