# Notes de Release — eMuleRust 0.1.3-lab

## 1. Vue d'Ensemble & Contexte

La version **0.1.3-lab** d'**eMuleRust** constitue le jalon d'intégration et de consolidation majeure du projet. Elle réunit :
1. **La complétion du protocole eD2K réel (Tier 3.1 à Tier 3.6)** validé contre l'exécutable officiel eMule 0.50a en environnement loopback hermétique.
2. **L'intégralité des fonctionnalités de Phase 8** : inspection détaillée, Part Map interactive, catégories locales avec hiérarchie de destination à 4 niveaux, historique SQLite avec rétention et exports caviardés, et import expérimental défensif de fichiers `.part.met` avec rapport d'audit.
3. **L'industrialisation et le packaging Windows certifiés** : deux saveurs étanches (`standard` et `qa-loopback`), validation de migration de schéma SQLite depuis Phase 7, portabilité vérifiée et matrice de supervision RC complète.

> **Déclaration Explicite de Confinement & Attestation d'Interopérabilité :**  
> *« eMuleRust est une application de laboratoire locale. Les transferts eD2K, la reprise, la vérification par parties et la finalisation ont été validés contre la référence eMule 0.50a dans un environnement strictement confiné à 127.0.0.1. Aucun serveur eD2K public, nœud Kad, peer externe, trafic UDP ni compression de protocole n’est activé. »*

---

## 2. Nouveautés & Évolutions Majeures

### 2.1 Protocole eD2K Loopback Réel & Interopérabilité eMule 0.50a (Tier 3.1 – 3.6)
- **Handshake eD2K canonique & Négociation** : Handshake complet eD2K avec support des tags de capacités eMule (`ET_MOD_VERSION`, `ET_VERSION`, `ET_EXT_FEATURES`).
- **Demande et Déclaration de Statut de Fichier** : Décodage et encodage des messages `FILE_STATUS_REQ` et `FILE_STATUS` avec bitfield par bloc et gestion de source complète (0 part requise).
- **Planification Binaire Bornée & Séquençage** : Décomposition déterministe en blocs de 9 728 000 octets et sous-plages de requêtes `REQUESTPARTS` (maximum 3 plages contiguës de 10 Ko).
- **Réception et Écriture en Flux (`SENDINGPART`)** : Écriture séquentielle et résiliente des blocs dans le fichier `.part` temporaire sans débordement de mémoire.
- **Contrôle Cryptographique MD4 & Hash Racine eD2K** : Hachage à la volée par bloc de 9,28 Mo et calcul de l'empreinte racine eD2K globale.
- **Résidu de Fin de Fichier & Promotion Atomique** : Prise en charge des transferts de frontière avec résidu de 1 octet (ex. fixture de 9 728 001 octets) et renommage atomique vers le dossier `downloads/`.
- **Reprise après Crash Forcé** : Démonstration de reprise sans corruption après arrêt brutal à ~50% de transfert, avec préservation de l'état SQLite et des blocs déjà validés.

### 2.2 Vue Détaillée, Part Map & Diagnostics Assainis (Phase 8.1)
- **`DownloadDetailDialog`** : Accès instantané aux métadonnées eD2K (UUID, nom, hash MD4, lien canonique `ed2k://`, taille totale, taille transférée, octets vérifiés).
- **Part Map eD2K Interactive** : Découpage visuel en blocs de 9,28 Mo avec code couleur unifié (Vérifié, Complété, En cours, Manquant) et info-bulles interactives avec offsets exacts.
- **Diagnostics Désensibilisés** : Génération d'un rapport de diagnostic expurgé de tout jeton d'authentification (`auth.token`) et anonymisant les répertoires locaux (`[DOWNLOAD_DIR]`).
- **Modification de Destination Sécurisée** : Boîte de dialogue dédiée avec sonde d'écriture (`write-probe`) validant les droits du système de fichiers avant confirmation.

### 2.3 Historique Local, Statistiques & Rétention Automatique (Phase 8.2)
- **Table SQLite `download_history`** : Enregistrement immuable de chaque événement de cycle de vie (`completed`, `failed`, `removed`, `resumed`, `verified_pending_promotion`).
- **Tableau de bord `HistoryView`** : Suivi des volumes et comptages sur 4 fenêtres temporelles distinctes (Cumul total, 7 derniers jours, 24 dernières heures, Session courante).
- **Politique de Rétention Configurable** : Élagage automatique configurable via l'interface (7, 30, 90 jours ou conservation indéfinie).
- **Exports Caviardés QA** : Export des événements au format JSON ou CSV avec anonymisation des chemins utilisateurs pour partage de logs sécurisé.

### 2.4 Catégories Locales & Résolution Hiérarchique de Destination (Phase 8.3)
- **Organisation Visuelle par Catégories** : Attribution de couleurs hexadécimales, priorités par défaut et répertoires de destination spécifiques.
- **Garantie Absolue de Non-Destruction** : La suppression d'une catégorie réassocie automatiquement les téléchargements avec `category_id = NULL` (rejoignant la vue « Sans catégorie ») sans jamais toucher aux fichiers.
- **Résolution Cible à 4 Niveaux** :
  $$\text{targetDirectory (tâche)} > \text{defaultDestination (catégorie)} > \text{defaultDownloadDir (global SQLite)} > \text{downloads/ (bundle portable)}$$

### 2.5 Import Défensif de Fichiers `.part.met` eMule (Phase 8.4)
- **Formats Supportés** : Décodage robuste des en-têtes standard eMule (`0xE0`), grands fichiers (`0xE1`) et nouveaux formats (`0xE2`).
- **Inversion des Balises de Gaps** : Calcul algorithmique du complément des lacunes pour identifier précisément les plages d'octets valides sans supposer de continuité arbitraire.
- **Vérification Cryptographique MD4 par Partie** : Comparaison systématique de chaque partie de 9,28 Mo contre le tableau de hachage du `.part.met`. Les blocs corrompus sont invalidés et reclassés en trous.
- **Confinement & Zéro Trafic Réseau** :
  - Le fichier source de l'utilisateur n'est jamais modifié ni déplacé.
  - Les données sont copiées sous `data/temp/<id>.part`.
  - Le téléchargement est initialisé en état **`Paused`** avec `sourceCount: 0`. Aucune tentative de connexion réseau n'est effectuée.
- **Rapport d'Audit d'Importation** : Affichage post-import d'un audit complet avec statistiques lisibles, bouton de copie dans le presse-papier et clôture explicite.

### 2.6 Packaging Multi-Saveurs, SBOM & Migration SQLite (Phase 8.5)
- **Deux Saveurs Windows** :
  - **Standard (`standard`)** : Épurée de tout code de test (`lab-tools` désactivé, méthodes `lab.*` renvoyant `-32601`).
  - **QA Loopback (`qa-loopback`)** : Équipée des outils d'injection de fixtures et du serveur de référence émulé sur `127.0.0.1`.
- **Migration de Schéma SQLite Idempotente** : Compatibilité descendante certifiée ; ouverture transparente des bases de données Phase 7 avec ajout sans perte de la colonne `category_id` et des nouvelles tables.
- **Portabilité & Mise à Niveau** : Écrasement de bundle binaire testé sans altération des données sous `data/` et `downloads/`.

---

## 3. Spécifications & Empreintes des Archives Livrables

### 3.1 Archives Officielles v0.1.3-lab

| Saveur | Fichier Archive ZIP | Taille (octets) | Empreinte SHA-256 |
| :--- | :--- | :--- | :--- |
| **Standard** | `eMuleRust-v0.1.3-lab-standard-windows-x64.zip` | 14 830 472 | `406F8A8DB03389D43DE20ECDC6AE7638226194B51C4422CEAE0D21D96FA3EB25` |
| **QA Loopback** | `eMuleRust-v0.1.3-lab-qa-loopback-windows-x64.zip` | 15 036 190 | `EDF14B8ABA85C90E393C01B8B988E984CC9010F5B365C55407206CA327A1DC70` |

### 3.2 Composition et Concordance Quadripartite
Chaque archive contient **22 fichiers** rigoureusement contrôlés, vérifiés conformes à 100% entre les fichiers physiques, `version.json`, `SHA256SUMS.txt` et le SBOM CycloneDX 1.5 (`sbom/sbom.cdx.json` regroupant 240 composants audités contre `deny.toml`).

---

## 4. Attestation de Référence eMule 0.50a (Tier 3)

Le binaire officiel de référence eMule 0.50a utilisé pour la qualification locale et le durcissement du protocole est épinglé par son empreinte cryptographique :
- **Fichier exécutable** : `emule.exe` (issu de `eMule0.50a.zip`)
- **Empreinte SHA-256** : `06cdf814387f627a4bd05a0c68211f715bfa952423e8e8a462e1f47c11a4d20e`
- **Politique de distribution** : Conformément aux règles de confidentialité et de confinement de laboratoire, l'exécutable eMule externe, ses répertoires de configuration et ses traces brutes ne sont pas distribués dans le dépôt Git ni dans les archives de release. Seules les instructions de test d'interopérabilité loopback sous variable d'environnement (`$env:EMULE_REFERENCE_BIN`) sont documentées dans `docs/walkthroughTier3.md`.

---

## 5. Synthèse de Qualification QA

| Banc d'Essai | Outil / Commande | Résultat Certifié |
| :--- | :--- | :--- |
| **Tests Unitaires & Intégration Rust** | `cargo test --workspace` | **151 / 151 passés (100%)** |
| **Analyse Statique Flutter** | `flutter analyze` | **0 avertissement, 0 erreur, 0 lint** |
| **Tests Widgets & Modèles Flutter** | `flutter test` | **74 / 74 passés (100%)** |
| **Smoke Test Standard Bundle** | `scripts/test_packaged_bundle.ps1 -Flavor standard` | **Conforme & validé en sandbox** |
| **Smoke Test QA Loopback Bundle** | `scripts/test_packaged_bundle.ps1 -Flavor qa-loopback` | **Conforme & validé en sandbox** |
| **Pipeline E2E QA Loopback** | `scripts/test_qa_loopback_e2e.ps1` | **7 / 7 étapes passées avec succès** |
| **Matrice Supervision & Environnement RC** | `scripts/test_rc_matrix.ps1` | **7 / 7 scénarios validés sans régression** |
| **Migration de Schéma SQLite** | `sqlite_categories_tests::test_phase7_to_phase8_schema_migration` | **Migration transparente, zéro perte** |
