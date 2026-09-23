# Notes de Version — Emule-Sion v0.2.0-RC1

**Date de publication :** 23 Septembre 2026  
**Statut :** Release Candidate — alignement comportemental eMule 0.72a, UX & résilience Kad

---

## 1. Synthèse de la Version

La version **v0.2.0-RC1** conclut un cycle d'audit complet du démon et de
l'interface : suppression des bridages internes hérités des premières
versions de test, amorçage Kad rendu résilient à la manière d'eMule, et
consolidation de l'expérience utilisateur (mesures de débit réelles,
onglet Partages enrichi, localisation française exhaustive).

1. **Bridages internes levés — alignement eMule 0.72a** :
   - Slots d'upload simultanés portés de 10 à **100**
     (`MAX_UP_CLIENTS_ALLOWED`, `Opcodes.h`) — le plafond précédent
     brisait le scaling sur connexions rapides.
   - Les **4 premiers slots** s'ouvrent sans condition de débit
     (`AcceptNewClient` : `max(MIN_UP_CLIENTS_ALLOWED, 4)`).
   - `promote_waiting` partage désormais l'éligibilité
     `can_open_slot` (plancher de débit + veto USS).
   - Pacing des connexions sources : `MAX_CONNECT_PER_TICK` 5 → 12
     (~4 connexions/s, rythme `MaxConnPer5Sec`=20 d'eMule) ; le budget
     token-bucket `rate_limit_per_min` (240/min) reste le garde-fou.
   - Cap inbound : la constante morte `MAX_INBOUND_CONNECTIONS` (500)
     est supprimée — le listener suit le budget `maxConnections` live ;
     le cap anti-flood par IP (5) est conservé.
   - Soft-cap Source Exchange : le réglage persisté
     `maxPeersPerDownload` remplace le fallback en dur à 400
     (`GetMaxSourcePerFile` → `thePrefs.maxsourceperfile`).

2. **Amorçage Kad résilient — `s_liBootstrapList`** :
   - Un timeout sur le premier bootstrap laissait Kad mort avec une
     table vide — la maintenance ne tournait que si
     `total_contacts > 0`.
   - Nouvelle file `bootstrap_candidates` (cap 50, dédupliquée,
     `kad_version >= 2`) alimentée par chaque handshake eD2K.
   - Drain en tâche de fond : 1 `BOOTSTRAP_REQ` toutes les **2 s** tant
     que la table est vide, **15 s** sinon — `CKademlia::Process`
     (Kademlia.cpp:287). Gate fidèle `HasHadContact` : le drain tourne
     tant qu'aucun contact *vérifié* n'a répondu.
   - `nodes.dat` (`<data_dir>/nodes.dat`) auto-chargé au démarrage.
   - `kad.statusChanged` émis dès les premiers contacts appris.

3. **USS aligné sur eMule** : *Last Common Route Finder* par
   traceroute ICMP Windows (`uss_icmp.rs`) et sélection du dernier
   routeur commun — mesure de la marge d'upload fidèle à
   `UploadBandwidthThrottler`.

4. **Débits d'upload réellement mesurés** : la vitesse de partage est
   calculée sur les octets effectivement émis (par slot et agrégée),
   rafraîchie en continu et décroissant vers zéro en l'absence
   d'activité — réutilisée par l'UI, les snapshots RPC, les stats du
   démon, l'USS et la file de trickle.

5. **Onglet Partages enrichi** :
   - **Catégorie par fichier partagé** (`shared_files.category_id`,
     migration idempotente, RPC `shares.setCategory`) — un partage issu
     d'un téléchargement suit la catégorie de celui-ci.
   - Groupage par catégorie (ordre `sortOrder`), « Sans catégorie » en
     dernier, tri interne par **total uploadé décroissant**.
   - Moteur de recherche (nom + hash eD2K), stats `↑ débit` / `Σ total`
     par fichier, priorité d'upload eMule (`verylow`…`powershare`).

6. **UI téléchargements** :
   - Chip de priorité modernisée : pill teintée, icônes sémantiques,
     3 couleurs (ambre = High, bleu ciel = Normal, ardoise = Low).
   - Barre de progression dédupliquée : `PartAvailabilityBar` porte
     seule l'avancement (liseré vert eMule + chunks), portée à 14 px.

7. **Audit des réglages** : inventaire des 36 clés RPC — chaque champ
   est câblé, borné (`maxConnections` ≤ 65 000, `maxUploadQueue`
   clampé avant persistance) et sans doublon entre sections.

8. **Liste des sources alignée sur `srclist`/deadlist eMule** :
   cooldowns, rejets (FNF) et filtrés IP masqués (deadlist invisible) ;
   `sourceCount` ne compte que les sources visibles.

9. **Localisation française exhaustive** : passage fichier par fichier,
   terminologie eMule naturelle.

---

## 2. Commandes de Validation

```powershell
# Vérification complète de compilation et des lints
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Suite de tests Rust (100% hors-ligne)
cargo test --workspace --all-features

# Validation de l'application Flutter Desktop
cd apps/desktop
flutter analyze
flutter test
```

---

## 3. Migrations

- **`shared_files.category_id`** : colonne ajoutée par `ALTER TABLE`
  idempotent au démarrage — aucune action manuelle requise, les bases
  existantes sont migrées en place.
- **`nodes.dat`** : déposer le fichier dans `<data_dir>` suffit — il est
  chargé automatiquement au boot en candidats de bootstrap.

---

## 4. Notes de Sécurité

- Aucune méthode `lab.*` dans le flavor `standard` — le bundle packagé
  est vérifié par `test_packaged_bundle.ps1`.
- Pipeline anti-SSRF, isolation loopback, kill switch atomique et
  pseudonymisation des pairs (`mule-network-policy`) inchangés.
