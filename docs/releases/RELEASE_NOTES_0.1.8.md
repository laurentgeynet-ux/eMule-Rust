# Notes de Version — EMule-Rust v0.1.8

**Date de publication :** 19 Septembre 2026  
**Statut :** Release de Performance, Stabilité d'Essaim & Observabilité

---

## 1. Synthèse de la Version

La version **v0.1.8** d'EMule-Rust apporte des optimisations déterminantes pour le téléchargement multi-pairs en essaim réel face à des clients eMule officiels, le respect strict des mécanismes de files d'attente distantes (*Queue Ranking*), l'équité des téléversements et une observabilité de niveau industriel :

1. **Agrégation Multi-Pairs du Débit (`sync_transfer_rates`)** :
   - Fin des chutes brutales de débit à 0 B/s : la vitesse de téléchargement par fichier est calculée dynamiquement comme la somme instantanée de tous les pairs actifs transférant des blocs pour ce hash.
   - `RateResetGuard` : lors de la déconnexion ou du départ d'un pair, le débit résiduel est immédiatement recalculé sans mise à zéro intempestive du fichier.

2. **Gestion Conforme des Files d'Attente Distantes (Queue Ranking eMule)** :
   - Détection de l'état `Disconnected in queue` : lorsqu'un client eMule distant ferme la socket TCP après nous avoir attribué un rang (`OP_QUEUERANKING`), notre client maintient la source en statut `Allowed` au lieu de la classer en erreur.
   - Respect scrupuleux du délai de réinterrogation canonique eMule (10 à 15 minutes) au lieu de reconnexions agressives à 60 secondes, évitant tout bannissement pour flood chez les pairs.

3. **Protection des Quotas de Swarm (Water-Filling)** :
   - Les pairs placés en attente dans les files distantes sont intégrés dans le décompte des sources viables, maintenant stable le plafond d'allocation water-filling.

4. **Gestionnaire de Téléversement (Upload Queue)** :
   - Prise en charge complète des requêtes entrantes `OP_STARTUPLOADREQ` et `OP_ACCEPTUPLOADREQ`.
   - Attribution équitable des slots d'envoi selon les crédits eMule accumulés (multiplicateur jusqu'à 10x).

5. **Système de Logs Stratégiques Normalisé en 5 Catégories** :
   - `[PART_COMPLETED]` : Notification lors de la complétion d'une partie de 9,28 Mo.
   - `[DL_PROGRESS]` : Rapport synthétique toutes les 10 secondes par fichier actif.
   - `[KAD_SEARCH]` : Télémétrie de recherche de sources et de mots-clés DHT.
   - `[UPLOAD_...]` : Cycle de vie des slots d'envoi (`REQUEST`, `GRANTED`, `QUEUED`, `RELEASED`, `PROMOTED`).
   - `[SYSTEM_HEARTBEAT]` : Synthèse consolidée de l'état du démon émise toutes les 60 secondes.

6. **Cartographie Exhaustive du Dépôt** :
   - Fichier [`docs/INDEX.md`](file:///d:/Projet/EMule-Rust/docs/INDEX.md) inventoriant et documentant chaque fichier versionné du projet.

---

## 2. Commandes de Validation

```powershell
# Vérification complète de compilation et des lints
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Exécution de la suite de tests Rust (204+ tests)
cargo test --workspace --all-features

# Validation de l'application Flutter Desktop
cd apps/desktop
flutter analyze
flutter test
```
