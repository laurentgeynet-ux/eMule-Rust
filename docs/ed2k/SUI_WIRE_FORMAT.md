# Secure User Identification (SUI) — Wire Format & Compatibility Specification

Status: **Phase A1 implémentée** — SUI bidirectionnel complet : vérification RSA réelle entrante, réponse aux challenges sortante, **et** émission de challenge sortante (rôle vérificateur) quand le pair annonce `SecIdent` dans MiscOptions1. Capacité `m_bySupportSecIdent` négociée dans les deux sens. Vecteurs binaires Tier 3 restant à produire. Voir §6 pour l'état d'implémentation.

## 1. Sources de référence (épinglées)

| Source | Fichier | Contenu utilisé |
|---|---|---|
| eMule officiel (repo `irwir/eMule`, master @ 2026-09) | `opcodes.h` | Opcodes, `RSAKEYSIZE` |
| eMule officiel | `ClientCredits.cpp` / `ClientCredits.h` | `CreateSignature`, `VerifyIdent`, `cryptkey.dat`, `CreditStruct` |
| eMule officiel | `BaseClient.cpp` | `SendPublicKeyPacket`, `SendSignaturePacket`, `ProcessPublicKeyPacket`, `ProcessSignaturePacket`, `SendSecIdentStatePacket`, `ProcessSecIdentStatePacket` |
| aMule (repo `amule-project/amule`, master) | `src/ClientCreditsList.cpp` | Confirmation croisée : schéma Crypto++ identique |

## 2. Findings confirmés

### Finding: schéma de signature — **RSA PKCS#1 v1.5 + SHA-1**, pas MD4 ni PSS

**Status:** Confirmed (eMule + aMule concordants)

- `m_pSignkey = new CryptoPP::RSASSA_PKCS1v15_SHA_Signer(filesource)` (eMule 0.72a `ClientCredits.cpp:404`, aMule `ClientCreditsList.cpp:InitalizeCrypting`)
- `CryptoPP::RSASSA_PKCS1v15_SHA_Verifier` pour la vérification.
- Taille de clé : `RSAKEYSIZE = 384` bits (eMule 0.72a `opcodes.h:96`, aMule `Constants.h:49`).
- Longueur de signature = longueur du module = **48 octets** pour RSA-384. La validation de taille doit être dérivée du module du pair (`modulus_len_bytes`), jamais une plage fixe `48..=96`.

**Implication eMule-Rust :** la crate `rsa` de RustCrypto supporte `Pkcs1v15Sign` + `Sha1`. ✅ Spike validé : `rsa 0.9.10` génère des clés 384 bits (`RsaPrivateKey::new`) et accepte `from_components` ; `Pkcs1v15Sign::new::<Sha1>()` exige l'entrée **pré-hachée** (20 octets) — voir `mule-crypto::legacy_sui`.

### Finding: contenu signé — la clé publique du *challenger* précède le challenge

**Status:** Confirmed

Contrairement à l'hypothèse "challenge + IP", le buffer signé est (eMule 0.72a `ClientCredits.cpp:447` `CClientCreditsList::CreateSignature` / aMule `ClientCreditsList.cpp:331-375`) :

```
V1 :  pubkey_challenger[keylen] || challenge u32 LE            → keylen + 4 octets
V2 :  pubkey_challenger[keylen] || challenge u32 LE
      || challenge_ip u32 LE || byChaIPKind u8                 → keylen + 9 octets
```

- `pubkey_challenger` = clé publique wire du pair qui a émis le challenge (côté signataire : `pTarget->GetSecureIdent()` ; côté vérificateur : `m_abyMyPublicKey`).
- `challenge` = `m_dwCryptRndChallengeFor` émis par le challenger dans `OP_SECIDENTSTATE` (reçu comme `m_dwCryptRndChallengeFrom` par le signataire).
- `challenge_ip` / `byChaIPKind` : voir §5.

**Implication :** `encode_emule_signed_payload` doit concaténer `pubkey || challenge || [ip || kind]`, dans cet ordre, LE pour les entiers.

### Finding: format `cryptkey.dat` — DER + Base64, pas un blob binaire

**Status:** Confirmed (eMule 0.72a `ClientCredits.cpp:422` `CClientCreditsList::CreateKeyPair`, aMule `ClientCreditsList.cpp:CreateKeyPair`)

- Génération : `InvertibleRSAFunction::Initialize(rng, 384)` puis `privkey.DEREncode()` dans un `Base64Encoder` → **le fichier est du texte Base64 contenant le DER Crypto++ de la clé privée** (`RSASSA_PKCS1v15_SHA_Signer` = `InvertibleRSAFunction` sérialisée : n, e, d, p, q, dp, dq, u en BER).
- Lecture : `FileSource(..., true, new Base64Decoder)` → decode Base64 puis parse BER/DER.
- Clé publique wire : `pubkey.DEREncode(asink)` dans un buffer de 80 octets = **structure BER `SEQUENCE { n INTEGER, e INTEGER }`** (CryptoPP `GetMaterial().Save` produit le même encodage). C'est cette forme DER (≤ 80 octets) qui circule dans `OP_PUBLICKEY` et qui est hachée pour l'identité de crédit.
- Comportement fichier vide → régénération ; absent → `CreateKeyPair`. La politique eMule-Rust (quarantaine au lieu d'écrasement) reste plus sûre et est conservée.

### Finding: `OP_SECIDENTSTATE` — sémantique du byte `state`

**Status:** Confirmed (eMule 0.72a `BaseClient.cpp:2076-2143` `SendSecIdentStatePacket`/`ProcessSecIdentStatePacket`)

Wire : `<state u8><challenge u32 LE>` — **nSize doit être exactement 5**, sinon rejet silencieux.

| `state` | Enum interne | Signification pour le *récepteur* du paquet |
|---|---|---|
| 0 | `IS_UNAVAILABLE` | Le challenger ne supporte pas SUI — abandonner |
| 1 | `IS_SIGNATURENEEDED` | Le challenger a déjà notre clé publique : envoyer `OP_SIGNATURE` seul |
| 2 | `IS_KEYANDSIGNEEDED` | Le challenger n'a pas notre clé : envoyer `OP_PUBLICKEY` puis `OP_SIGNATURE` |

- Émis seulement si `CryptoAvailable()` et (`SecIDKeyLen == 0` → 2, ou `m_dwLastSignatureIP != GetIP()` → 1). Si `nValue == 0`, **aucun paquet n'est envoyé** (pas de state 0 explicite sauf crypto indisponible côté challenger → en fait 0 n'est jamais émis dans ce chemin ; `m_SecureIdentState = IS_UNAVAILABLE` est aussi la valeur initiale).
- Challenge généré par `rand()+1` côté eMule — **non cryptographiquement sûr** ; eMule-Rust doit utiliser `OsRng` tout en restant compatible (c'est juste un u32 à relire).

### Finding: `OP_SIGNATURE` V2 — byte `byChaIPKind` en queue

**Status:** Confirmed (eMule 0.72a `BaseClient.cpp:1949-1980` envoi, `:2016-2035` vérification dont `:2028` pour la forme V2)

```
OP_SIGNATURE : <siglen u8><signature[siglen]>            ← V1 : pBuffer[0] == nSize-1
OP_SIGNATURE : <siglen u8><signature[siglen]><kind u8>   ← V2 : pBuffer[0] == nSize-2
```

- V2 n'est acceptée que si `(m_bySupportSecIdent & 2) != 0` (capacité annoncée dans les info-packets ; la valeur du tag = 3 quand `CryptoAvailable()`, bits : `&1` = v1, `&2` = v2).
- V1 est le défaut ; V2 n'est utilisé que si le pair ne supporte pas V1 (`(m_bySupportSecIdent & 1) == 1 → bUseV2 = false`).
- `byChaIPKind` : `CRYPT_CIP_REMOTECLIENT=10` (IP du pair utilisée — cas LowID), `CRYPT_CIP_LOCALCLIENT=20` (notre client ID HighID), `CRYPT_CIP_NONECLIENT=30` (IP=0).

### Finding: liaison IP (`useIP`)

**Status:** Confirmed (eMule `ClientCredits.cpp:VerifyIdent`)

- `CRYPT_CIP_LOCALCLIENT` : `ChallengeIP = dwForIP` = IP du signataire telle qu'observée par le vérificateur.
- `CRYPT_CIP_REMOTECLIENT` : `ChallengeIP =` IP publique du vérificateur (fallback `GetED2KID()` si LowID sans IP publique connue).
- Après vérification : `Verified(dwForIP)` stocke `m_dwIdentIP = dwForIP` ; un pair identifié qui change d'IP devient `IS_IDBADGUY` (crédits gelés) jusqu'à re-identification.
- Anti-flood : **une seule signature acceptée par IP** (`m_dwLastSignatureIP == GetIP()` → drop).

### Finding: indexation des crédits

**Status:** Confirmed — correction d'une hypothèse antérieure

- `GetCredit(m_achUserHash)` : la table `clients.met` est indexée par le **user_hash MD4** du pair (16 octets), pas par le hash de la clé publique.
- La clé publique vérifiée est stockée *dans* le `CreditStruct` (`abySecureIdent[80]` + `nKeySize`) : la première vérification épingle la clé ; `SetSecureIdent` refuse tout changement de clé une fois `nKeySize != 0`.
- Effet de sécurité : si un pair déjà crédité présente une nouvelle clé, `VerifyIdent` échouera (clé épinglée ≠ signature) → `IS_IDFAILED`, et lors d'une nouvelle épingle, les crédits antérieurs sont remis à 1/1 (`Verified()`).

## 3. Bug interop corrigé

~~`crates/mule-protocol/src/ed2k/opcodes.rs` inverse les deux opcodes officiels~~ — **corrigé** : `OP_SIGNATURE = 0x86`, `OP_SECIDENTSTATE = 0x87`, alignés sur `opcodes.h:259-260` (eMule 0.72a). Constantes, commentaires encodeur/décodeur et tests roundtrip mis à jour (46/46 verts).

## 4. Format des messages wire (confirmé)

```
OP_PUBLICKEY     0x85 : <keylen u8><pubkey_der[keylen]>         (keylen ≤ 80, nSize ∈ [10..250])
OP_SECIDENTSTATE 0x87 : <state u8><challenge u32 LE>            (nSize == 5 strictement)
OP_SIGNATURE     0x86 : <siglen u8><sig[siglen]>[<kind u8>]     (nSize ∈ [10..250])
```

## 5. État d'implémentation (A1)

### `mule-crypto` (nouveau crate)

| Module | Rôle |
|---|---|
| `legacy_sui.rs` | `build_sui_signing_payload` (V1/V2), `sign_sui_payload`, `verify_sui_signature` (RSASSA-PKCS1v15-SHA1, taille dérivée du module), `parse_peer_public_key` (PKCS#1 DER ≤ 80 o) |
| `cryptkey.rs` | `cryptkey.dat` : Base64→DER PKCS#1→RSA-384 ; chargement `Loaded/Missing/Invalid/LegacyBlob`, quarantaine horodatée, écriture atomique (close avant rename, Windows-safe) |

#### Migration du blob legacy 80 octets

Les anciennes versions du daemon écrivaient un `cryptkey.dat` de **80 octets binaires aléatoires** (aucune clé RSA récupérable). `load_cryptkey` le distingue d'une vraie corruption : exactement 80 octets **et** présence d'octets hors alphabet Base64 → `LegacyKeyLoad::LegacyBlob`. Le fichier est quarantiné pareil, puis `IdentityService` **génère une nouvelle clé RSA-384** avec un `WARN` de migration explicite — l'ancien blob n'était pas une identité réelle, rien à préserver. Un fichier de 80 octets *texte* (alphabet Base64 uniquement) reste `Invalid` classique (SUI désactivé, pas de régénération). Cas réel vérifié : mise à niveau par écrasement avec `data/` conservé.

### Inbound (`mule-daemon::inbound_listener::dispatcher`)

- `OP_PUBLICKEY` : clé ≤ 80 o stockée ; challenge `u32` émis **uniquement si identité locale utilisable** ; aucun crédit avant vérification.
- `OP_SECIDENTSTATE` : **rôle signataire entrant** — le pair entrant peut aussi *nous* challenger. Réponse = notre `OP_PUBLICKEY` si `state=2`, puis `OP_SIGNATURE` V1 via `IdentityService` (injecté par `set_identity`). Si le challenge arrive **avant** l'`OP_PUBLICKEY` du pair, il est mis en attente (`pending_peer_challenge`) et traité dès réception de la clé — l'ordre des paquets n'est pas garanti chez eMule (observé sur le réseau réel : plusieurs pairs envoient le challenge en premier).
- `OP_SIGNATURE` : vérification RSA réelle — challenge consommé dans tous les cas (anti-rejeu), anti-flood une signature/IP (`last_sig_ip`), byte `kind` V2 extrait (`10`→IP locale, `20`→IP du pair, `30`→0, autre→rejet), payload V1/V2 reconstruit puis `verify_sui_signature`.
- Succès → clé épinglée à l'enregistrement de crédit + `credit_multiplier` ; échec → `SecIdentState(0)` + état purgé, `WARN` avec motif.

### Outbound (`mule-daemon::peer_session::sui::OutboundSui`)

- **Négociation** : `SecIdent=3` annoncé dans MiscOptions1 bits 16-19 du HELLO sortant et du HELLO_ANSWER entrant (0 si `cryptkey.dat` absent/en quarantaine), conforme `BaseClient.cpp:1010` (`CryptoAvailable() ? 3 : 0`). La capacité du pair est exposée via `Ed2kPeerInfo::secure_ident_support()`.
- **Signataire** : branché dans les boucles status/queue/transfer — mémorise `OP_PUBLICKEY` du pair, répond à `OP_SECIDENTSTATE` (`state=2` → notre `OP_PUBLICKEY` d'abord, puis `OP_SIGNATURE` V1 via `IdentityService::sign_sui_payload`). Une seule réponse par connexion. **Ordre des paquets toléré** : un challenge reçu avant l'`OP_PUBLICKEY` du pair est différé (`pending_challenge`) puis traité à réception de la clé — bug interop corrigé après observation de vrais clients eMule émettant `state=2` immédiatement après le handshake.
- **Vérificateur** : après le handshake, si le pair a annoncé `SecIdent != 0` et qu'on a une clé, on émet `OP_SECIDENTSTATE(state=2, challenge)` (eMule `InfoPacketsReceived`/`SendSecIdentStatePacket`). `OP_SIGNATURE` reçu → payload `our_pubkey || challenge [|| ip || kind]` vérifié avec la clé du pair, kind V2 géré (`20`→IP du pair, `10`→IP locale, `30`→0). Résultat loggé `INFO`/`WARN`.

### Observabilité (`SuiStats`)

Compteurs atomiques partagés (`mule-daemon::services::identity::SuiStats`), incrémentés inbound et outbound, exposés en lecture seule via RPC `identity.status` sous l'objet `sui` : `challengesIssuedIn`, `challengesIssuedOut`, `signaturesSent`, `signaturesVerified`, `signaturesRejected`, `signaturesRejectedFlood`, `malformedPackets`. Aucune donnée privée (clé, challenge) n'est exposée.

### Tests

- `test_inbound_secure_ident_verified_with_real_rsa` : RSA-384 réel → crédit créé + octets uploadés attribués.
- `test_outbound_secure_ident_answers_peer_challenge` : mock eMule vérifie cryptographiquement notre signature.
- `test_outbound_secure_ident_challenges_peer_when_advertised` : le pair annonce `SecIdent=3` → le daemon émet `SecIdentState(state=2)` non sollicité.
- Anciens tests à signatures factices : toujours rejetés (vérification réelle).

### Réserves Tier 3 (à confirmer contre eMule/aMule réel)

- ✅ **Challenge avant `OP_PUBLICKEY` (ordre des paquets)** : confirmé sur réseau réel — plusieurs pairs eMule émettent `OP_SECIDENTSTATE(state=2)` immédiatement après le handshake, avant leur clé. Le signataire doit différer le challenge jusqu'à réception de la clé (implémenté inbound + outbound).
- ✅ **Format `cryptkey.dat` legacy eMule-Rust** : confirmé — l'ancien fichier de 80 octets binaires n'est pas du Base64/DER ; détecté comme `LegacyBlob` et migré (quarantaine + nouvelle clé).

- Endianness `ip_u32` V2 (`from_be_bytes` + LE actuellement).
- `kind=10` utilise l'IP du socket local ; eMule utilise l'IP publique — LowID/CGNAT non validé.
- Endianness exacte de `dwIP` dans `VerifyIdent` (convention eD2K inversée possible).
- `cryptkey.dat` écrit par `rsa` est du PKCS#1 standard ; concordance octet-par-octet avec Crypto++ `DEREncode` (ordre des composantes, version `0`/`1` two-prime) à confirmer par fixture réelle.

## 6. Pending / à compléter en A0

- [ ] Vecteurs hexadécimaux : capture Tier 3 eMule 0.72a (V1 + V2, HighID et LowID).
- [ ] Fixture `cryptkey.dat` réel produit par eMule, parsée octet par octet (ordre exact des composantes BER dans `InvertibleRSAFunction::DEREncode`).
- [x] Tag de capacité `m_bySupportSecIdent` : MiscOptions1 bits 16-19, valeur `3` (v1|v2) — annoncé et parsé (`Ed2kPeerInfo::secure_ident_support`). Reste : n'accepter les signatures V2 entrantes que si le pair a annoncé `& 2` (BaseClient.cpp:2028).
- [ ] Comportement `ProcessSecIdentStatePacket` pour `state > 2` (non géré → challenge lu quand même) — à trancher.
- [ ] Fuzzing des bornes `nSize < 10 || nSize > 250` observées côté eMule.
