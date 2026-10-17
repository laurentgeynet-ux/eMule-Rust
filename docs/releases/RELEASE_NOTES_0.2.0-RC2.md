# Notes de Version — eMuleRust v0.2.0-RC2

**Date de publication :** 25 Septembre 2026  
**Statut :** Release Candidate — audit filaire/comportemental eMule 0.72a
exhaustif, interopérabilité live prouvée, migration eMule complète

---

## 1. Synthèse de la Version

La version **v0.2.0-RC2** clôt une campagne d'audit de bout en bout du
filaire et du comportement contre les sources officielles d'eMule 0.72a
(`srchybrid`) : couverture exhaustive des opcodes TCP/UDP pair et serveur,
anti-abuse fidèle (`CLIENTBANTIME`, `BADCLIENTBAN`,
`ComparePriorUserhash`), file d'upload persistante aux déconnexions, et —
point culminant — **validation live contre le vrai binaire eMule 0.72a** :
7/7 tests d'interopérabilité, dont des transferts de blocs compressés
réels.

1. **Interop live eMule 0.72a : 7/7** — le harnais Tier-3 lance le
   `emule.exe` officiel (SHA-256 épinglé, profil loopback isolé) et
   valide handshake, file-status, transferts de blocs, reprise et
   finalisation. Deux bugs filaires réels ont été trouvés et corrigés
   grâce à ces tests :
   - **`OP_COMPRESSEDPART` (0x40)** : le client de test sautait tout
     paquet `PR_EMULE` — or eMule y transporte les blocs zlib. Décodeur
     borné + réassemblage incrémental du flux compressé (tranches de
     ≤ 10 240 octets sur plusieurs paquets) + sous-plages validées.
   - **Bug de flux** : réception bloquante au lieu de drainer les parts
     décompressées déjà produites — deadlock sur transfert multi-paquets.
   - Correctif annexe : `COMPRESSEDPART{,_I64}` émis sous `PR_EMULE`
     (`OP_EMULEPROT`), pas `PR_EMULE_COMPRESSED`.

2. **Couverture opcodes quasi-totale** — diff mécanique des ~95 `case`
   d'eMule : browsing de partages, verdict pare-feu Kad TCP,
   `SERVER_DESC_REQ/RES`, `PUBLICIP_REQ`, `OUTOFPARTREQS`, `PACKEDPROT`
   pair + Kad UDP packé, `PORTTEST`, callbacks LowID
   (`REASKCALLBACKTCP`/`CALLBACK`/`DIRECTCALLBACKREQ`), hashsets
   `FileIdentifier` (`HASHSETREQUEST2/ANSWER2`), `QUEUERANK` legacy +
   `QUEUERANKING` en payload 12 octets `PR_EMULE`. Seul absent :
   `PREVIEWANSWER` — jamais sollicité puisque preview=0 est annoncé.

3. **Anti-abuse eMule 0.72a** : bans IP limités à `CLIENTBANTIME` (2 h)
   au lieu de permanents ; `AddRequestCount`/`BADCLIENTBAN` → vrai ban IP
   + déconnexion (exemption ami) ; `ComparePriorUserhash` sur `(IP, port)`
   suivi 2 h via SUI ; `ipfilter` appliqué aux datagrammes UDP pair et à
   l'accept TCP.

4. **File d'upload fidèle aux déconnexions** : une entrée en file survit
   à la mort du socket jusqu'à `MAX_PURGEQUEUETIME` (score intact) ;
   le meilleur injoignable dominant est marqué `add_next_connect` et sa
   reconnexion ouvre un slot immédiat — fidèle à
   `Disconnected`/`FindBestClientInQueue`/`AddClientToQueue`.

5. **Migration eMule** : parsers et imports `known.met`, `clients.met`
   (crédits), `emfriends.met`, `cancelled.met`, identité complète
   (cryptkey + userhash), assistant de migration dans Paramètres, et
   marquage « Annulé » des résultats de recherche
   (`IsCancelledFileByID`).

6. **Kad** : expressions de recherche restrictives
   (`StartSearchTermsMatch`), publication de mots-clés AICH, modèle
   `CKeyEntry`, identité eMule 0.72a.

7. **Comportement download** : purge des sources `RemoteQueueFull` à
   80 % de `maxSources`, A4AF au niveau socket (`SwapToAnotherFile`),
   cycle re-ask de file distante fidèle, fenêtre UDP `(1 s, 2 min)`
   pré-reask TCP.

8. **Qualité des tests** : mutualisation dans `mule-test-support` et
   `tests/common/`, garde-fou anti-doublons (`test_duplicates.ps1`).

---

## 2. Commandes de Validation

```powershell
# Vérification complète de compilation et des lints
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Suite de tests Rust (100% hors-ligne)
cargo test --workspace --all-features

# Interop live (optionnel — nécessite le binaire eMule épinglé)
$env:EMULE_REFERENCE_BIN = "D:\eMule0.72a\eMule0.72a\emule.exe"
$env:EMULE_REFERENCE_SHA256 = "<sha256 du binaire>"
$env:EMULE_REFERENCE_PROFILE = "Emule072a"
cargo test -p mule-protocol --test tier3_loopback --all-features -- --include-ignored

# Validation de l'application Flutter Desktop
cd apps/desktop
flutter analyze
flutter test
```

---

## 3. Migrations

- **`peer_credits`** : import `clients.met` vers la table SQLite —
  aucune action manuelle, migration en place au démarrage.
- **Identité eMule** : `cryptkey`/`userhash` importés — la réputation
  (crédits vus par les pairs) est conservée dès le premier démarrage.
- **Assistant de migration** : Paramètres → Avancé guide l'import des
  `.met` depuis une installation eMule existante.

---

## 4. Notes de Sécurité

- Les bans IP sont désormais **bornés à 2 h** (`CLIENTBANTIME`) —
  fidélité eMule ; le comportement est *moins* restrictif qu'avant sur
  la durée, mais *plus* strict sur la portée (bans propagés aux chemins
  TCP entrant et UDP pair).
- Aucune méthode `lab.*` dans le flavor `standard` — le bundle packagé
  est vérifié par `test_packaged_bundle.ps1`.
- Pipeline anti-SSRF, isolation loopback, kill switch atomique et
  pseudonymisation des pairs (`mule-network-policy`) inchangés ; les
  tests live Tier-3 restent confinés au loopback.
