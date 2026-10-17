# Notes de Version — eMuleRust v0.2.0-RC3

**Date de publication :** Octobre 2026
**Statut :** Release Candidate — rebrand complet **eMuleRust**, campagne
de validation pré-publication A–D entièrement verte, durcissement
anti-SSRF, premier dépôt public GitHub.

---

## 1. Synthèse de la Version

La version **v0.2.0-RC3** est la première release publiée sous le nom
définitif **eMuleRust** sur GitHub
(`laurentgeynet-ux/eMule-Rust`). Elle capitalise RC2 (audit filaire
eMule 0.72a exhaustif + interop live) et ajoute une campagne de
validation complète en 4 gates — tout est vert.

1. **Rebrand eMuleRust** — nom affiché, exécutables (`eMuleRust.exe`),
   bundles, variables d'environnement `EMULERUST_*` (alias dépréciés
   `EMULE_SION_*` conservés), identifiants Dart/Rust, docs bilingues
   (README anglais + `README.fr.md`).

2. **Campagne de validation A–D** :
   - **Phase A** : `verify_all.ps1` 100 % vert, bundle standard
     rebuildé et smoke-testé (aucune méthode `lab.*` exposée).
   - **Phase B** : banc fonctionnel RPC **134 PASS / 0 FAIL** —
     couverture de toute la surface publique (identité, downloads,
     formats `.met`/`.dat`, shares, sources, serveurs, Kad, filtres,
     settings, historique) + chemins d'erreur.
   - **Phase C** : interop live **7/7 × 3 références** — eMule 0.50a,
     eMule 0.72a et aMule 3.1.0 (binaires épinglés par SHA-256,
     sandbox hermétique, loopback strict).
   - **Phase D** : réseau réel supervisé **21/21** — import
     `server.met` réel (16 serveurs), session eD2K établie,
     95 contacts Kad, recherches eD2K + Kad réelles, kill switch,
     UPnP mappé puis unmap propre (< 500 ms).

3. **Durcissement anti-SSRF** : `servers.updateFromUrl` et
   `kad.bootstrapFromUrl` valident désormais schéma (http/https
   uniquement) et adresses résolues — rejet link-local, LAN privé,
   cibles interdites par la policy réseau.

4. **Correctifs de robustesse** :
   - Shutdown daemon borné : unmap UPnP détaché (`std::thread` +
     oneshot 500 ms) — plus de blocage > 5 s sur gateway lente.
   - Smoke test packagé : drainage continu des pipes du daemon.
   - E2E loopback : crash/reprise déterministe via `downloads.pause`.
   - Mutualisation des fixtures de test dans `mule-test-support`.

5. **CI/CD GitHub** : workflow `CI` (Rust check/clippy/test + Flutter
   analyze/test, daemon compilé pour les tests superviseur/E2E),
   workflow `Packages` (bundles Windows + Linux en artifacts),
   `release.yml` (build + smoke + publication GitHub Release sur tag
   `v*`), Dependabot actif.

## 2. Commandes de Validation

```powershell
# Vérification complète
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\verify_all.ps1

# Banc fonctionnel RPC (loopback, offline)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_phase_b_functional_bench.ps1

# Banc réseau réel (supervisé — émet du trafic eD2K/Kad public)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_phase_d_public_network.ps1

# Interop live vs références réelles (binaires épinglés)
cargo test -p mule-protocol --test tier3_loopback --all-features -- --include-ignored
```

## 3. Migrations

- **Variables d'environnement** : `EMULERUST_WORKDIR`,
  `EMULERUST_DOWNLOADS_DIR`, `EMULERUST_DEPLOY_DIR` sont prioritaires ;
  les anciennes `EMULE_SION_*` / `EMULE_DEPLOY_DIR` restent lues en
  fallback déprécié (aucune action requise).
- **UPnP** : les baux créés sous l'ancien préfixe `eMule-Rust` /
  `EMule-Rust` restent reconnus et unmapped à l'arrêt.
- **Workdir** : `%LOCALAPPDATA%\eMuleRust` est utilisé ; aucune
  migration automatique depuis `eMule-Sion` — réimporter via
  l'assistant de migration eMule si besoin.

## 4. Notes de Sécurité

- Anti-SSRF appliqué aux fetches d'URL (server.met / nodes.dat) —
  schéma + résolution DNS vérifiés contre la policy réseau.
- Aucune méthode `lab.*` dans le flavor `standard` — vérifié par
  `test_packaged_bundle.ps1` en CI.
- Branche `master` protégée : force-push et suppression bloqués.
- Builds non signés (Authenticode optionnel via secrets
  `WINDOWS_CERT_BASE64` / `WINDOWS_CERT_PASSWORD`).

## 5. Limites connues

- **Windows x64 uniquement** pour l'UI packagée (le bundle CI Linux
  est expérimental — premier build automatisé ajouté par cette RC).
- Session eD2K observée en **LowID** dans le banc Phase D malgré un
  mapping UPnP confirmé — comportement acceptable (dégradé), sous
  investigation.
- Pas de téléchargement en tâche de fond sur Android (contrainte OS) —
  la cible Android n'est pas maintenue.
