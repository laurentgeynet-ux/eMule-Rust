# Récapitulatif de session — Chiffrement et identité cryptographique EMule-Rust

Date : 2026-09-19
Périmètre : audit de l'existant, spécification wire SUI dérivée des sources eMule/aMule, corrections appliquées, architecture cible.

> **Note (mise à jour)** : la section 1 décrit l'état *avant* implémentation. Depuis, le SUI est implémenté en entier — vraie clé RSA-384 dans `cryptkey.dat` (avec migration automatique du blob legacy 80 octets via `LegacyKeyLoad::LegacyBlob`), signature/vérification réelles inbound et outbound, challenges différés quand `OP_SECIDENTSTATE` arrive avant `OP_PUBLICKEY`, et compteurs `SuiStats` exposés via `identity.status`. État courant : `SUI_WIRE_FORMAT.md` §5.

---

## 1. État actuel du code (audit)

### 1.1 `IdentityService` — identité factice

Fichier : `crates/mule-daemon/src/services/identity.rs`

- `cryptkey.dat` actuel = **80 octets aléatoires** (CSPRNG), magic bytes `0x01 0x00`. Aucune vraie paire RSA.
- La même valeur sert de "clé publique" (`public_key_bytes()`) et de base au `user_hash` (MD4 + magic bytes eMule `out[5]=14`, `out[14]=111`).
- `status()` annonce `key_bits: 384` — cosmétique, aucune opération RSA n'existe.
- Instancié dans `AppState` (`state.identity`), utilisé par `peer_session`, `server_session`, `inbound_listener` pour le `user_hash`.
- **Aucune signature sortante** : `public_key_bytes()` n'est appelé nulle part. Le daemon ne s'authentifie jamais.

### 1.2 Secure-ident entrant — simulation

Fichier : `crates/mule-daemon/src/services/inbound_listener/dispatcher.rs` (lignes ~56-135)

- `OP_PUBLICKEY` reçu → MD4 de la clé, lookup `get_peer_credit`, envoi `SecIdentState(state=1, challenge=rand u32)`.
- `OP_SIGNATURE` reçu → **"vérification" = test de longueur `(48..=96)` uniquement**. Aucune crypto. Un blob aléatoire de la bonne taille authentifie le pair et débloque `credit_multiplier`.
- Succès → `SecIdentState(state=2)` + multiplicateur appliqué à l'upload queue.

### 1.3 Couche protocole — correcte

`crates/mule-protocol/src/ed2k/` : encodeurs/décodeurs purs pour `OP_PUBLICKEY`, `OP_SIGNATURE`, `OP_SECIDENTSTATE` (`encoder/security.rs`, `decoder/security.rs`), types `Ed2kPublicKey`/`Ed2kSignature`/`Ed2kSecIdentState`, intégrés dans `Ed2kMessage` et la session. Pas de crypto dans les codecs — bonne séparation.

### 1.4 Bug corrigé cette session

**Opcodes inversés** dans `crates/mule-protocol/src/ed2k/opcodes.rs` :

| Opcode | eMule officiel (`opcodes.h`) | EMule-Rust (avant → après) |
|---|---|---|
| `OP_PUBLICKEY` | `0x85` | `0x85` ✓ |
| `OP_SIGNATURE` | **`0x86`** | `0x87` → `0x86` ✓ |
| `OP_SECIDENTSTATE` | **`0x87`** | `0x86` → `0x87` ✓ |

Correction appliquée : constantes + doc comments (`opcodes.rs`, `packet.rs`, `encoder.rs`, `encoder/security.rs`, `decoder.rs`, `decoder/security.rs`, commentaires de test). Suite `ed2k_roundtrip_tests` : **46/46 tests verts**.

---

## 2. Spécification wire SUI — confirmée depuis les sources

Référence complète : `docs/ed2k/SUI_WIRE_FORMAT.md`. Sources : repo `irwir/eMule` (`opcodes.h`, `ClientCredits.cpp`, `BaseClient.cpp`) et `amule-project/amule` (`ClientCreditsList.cpp`) — les deux implémentations concordent.

### 2.1 Schéma cryptographique

- **RSA-384 bits** (`RSAKEYSIZE = 384`), **PKCS#1 v1.5 + SHA-1** — `CryptoPP::RSASSA_PKCS1v15_SHA_Signer`/`Verifier`. Ni MD4, ni PSS, ni RSA brut.
- Signature = longueur du module : **48 octets** pour RSA-384. La validation de taille doit dériver du module du pair reçu, pas d'une plage fixe.

### 2.2 `cryptkey.dat` réel

- Clé privée `InvertibleRSAFunction` sérialisée en **DER puis encodée Base64** — fichier texte, pas binaire.
- Contenu DER : composantes `n, e, d, p, q, dp, dq, u` (BER sequence).
- Clé publique wire = `pubkey.DEREncode()` dans ≤ 80 octets = `SEQUENCE { n INTEGER, e INTEGER }`. C'est ce blob DER qui circule dans `OP_PUBLICKEY` et qui est épinglé dans les crédits.
- Politique EMule-Rust : fichier corrompu → **quarantaine horodatée**, jamais d'écrasement silencieux (perte de crédits historiques).

### 2.3 Contenu signé

```
V1 : pubkey_challenger[keylen] || challenge u32 LE                          → keylen + 4
V2 : pubkey_challenger[keylen] || challenge u32 LE || ip u32 LE || kind u8  → keylen + 9
```

- Le signataire signe **la clé publique du challenger** (liaison bilatérale), suivie du challenge reçu.
- `challenge` : u32, émis par le challenger dans `OP_SECIDENTSTATE` (`m_dwCryptRndChallengeFor` côté émetteur, `From` côté récepteur). eMule utilise `rand()+1` (non crypto) — EMule-Rust utilisera `OsRng`, compatible.

### 2.4 Messages wire

```
OP_PUBLICKEY     0x85 : <keylen u8><pubkey_der[keylen]>          nSize ∈ [10..250]
OP_SECIDENTSTATE 0x87 : <state u8><challenge u32 LE>             nSize == 5 strict
OP_SIGNATURE     0x86 : <siglen u8><sig[siglen]>                 V1 : siglen == nSize-1
                        <siglen u8><sig[siglen]><kind u8>        V2 : siglen == nSize-2
```

- `state` : 0=indisponible, 1=`IS_SIGNATURENEEDED` (clé déjà connue → signature seule), 2=`IS_KEYANDSIGNEEDED` (envoyer pubkey puis signature).
- `kind` (`byChaIPKind`) : `10=CRYPT_CIP_REMOTECLIENT` (IP du pair, cas LowID), `20=CRYPT_CIP_LOCALCLIENT` (notre client ID), `30=CRYPT_CIP_NONECLIENT` (IP=0).
- V2 utilisé seulement si le pair n'annonce pas V1 (`m_bySupportSecIdent & 1`). Capacité annoncée = valeur 3 dans les info-packets.

### 2.5 Liaison IP et anti-abus

- `LOCALCLIENT` : `ChallengeIP` = IP du signataire vue par le vérificateur ; `REMOTECLIENT` : IP publique du vérificateur (fallback `ED2KID` si LowID).
- `Verified(dwForIP)` épingle `m_dwIdentIP` : un pair identifié qui change d'IP → `IS_IDBADGUY` jusqu'à re-vérification.
- Anti-flood : une seule signature acceptée par IP (`m_dwLastSignatureIP`).

### 2.6 Crédits

- `clients.met` indexé par **user_hash MD4** du pair (`GetCredit(m_achUserHash)`).
- La clé publique vérifiée est stockée dans `CreditStruct.abySecureIdent[80]` + `nKeySize` — épinglage à la première vérification, `SetSecureIdent` refuse tout changement ultérieur.
- Nouvelle épingle sur un pair déjà crédité → remise à 1/1 des compteurs (`Verified()`).

---

## 3. Architecture cible

### 3.1 Principe directeur

Deux cryptographies séparées, jamais les mêmes clés :

| Besoin | Mécanisme | Rôle |
|---|---|---|
| Compatibilité eMule/aMule | **RSA-384 + PKCS1v15/SHA-1** (legacy SUI) | Prouver la possession du `user_hash`, préserver les crédits |
| Identité moderne EMule-Rust | **Ed25519** | Signatures des extensions propriétaires |
| Échange de clés / canaux | **X25519 + HKDF + ChaCha20-Poly1305** | Sessions éphémères chiffrées |
| Intégrité moderne | BLAKE3 / SHA-256 | Complément à MD4/AICH legacy |

RSA-384 est cryptographiquement faible : il ne sert **que** à l'interop SUI, jamais au chiffrement de fichiers/messages ni à la protection des clés modernes.

### 3.2 Répartition par crate

```
mule-crypto/                  # nouveau : primitives uniquement
  legacy_sui.rs               # RSA-384 eMule : clés, sign, verify
  modern_identity.rs          # Ed25519 (clé séparée, pas de conversion Ed→X)
  kdf.rs                      # HKDF, séparation de domaines

mule-format/
  cryptkey_dat.rs             # parse/écrit Base64+DER eMule, quarantaine

mule-protocol/
  ed2k/secure_ident.rs        # opcodes (corrigés), payloads V1/V2, state wire

mule-daemon/
  services/identity.rs        # IdentityBundle { legacy_sui, ed25519, user_hash }
  services/peer_session + inbound_listener
                            # machine d'états SUI partagée
```

Règles : les décodeurs restent purs (format seulement) ; `mule-crypto` est la seule frontière crypto ; le dispatcher décide du "quand".

### 3.3 Machine d'états SUI (partagée inbound/outbound)

`SecureIdentState` : `Unsupported / Idle / AwaitingPeerPublicKey / ChallengeSent / ChallengeReceived / AwaitingSignature / Verified / Failed`. Deux niveaux stricts : byte wire (`state u8 + challenge u32`) ≠ sémantique métier — `interpret_secure_ident_state()` fait la traduction, `todo!()` tant que la spec n'est pas validée.

### 3.4 Politique IP

`SecureIdentIpBinding` : `Required / DisabledForCgnat / DisabledForUnknownAddress / CompatibilityFallback` — décision via `mule-network-policy` (`is_cgnat_ip` existant), pas de comparaison naïve d'adresses dans le code SUI.

### 3.5 Canal moderne (Ed25519)

Jamais de surcharge silencieuse d'`OP_SIGNATURE` : négociation de capacité EMule-Rust explicite + sous-protocole versionné séparé. Les keypairs Ed25519 et X25519 sont **distincts** (la conversion directe de seed est incorrecte).

---

## 4. Contexte : routage en oignon (analyse antérieure)

Objectif plus large : chiffrer les transmissions et anonymiser via routage multi-sauts inspiré Tor/I2P.

**Retenu** :
- KAD reste en clair pour la découverte ; l'oignon s'applique au transport.
- Double découpage : parts de 9,28 Mo logiques / chunks réseau (≤ 64 Ko), padding à taille fixe.
- Streaming via `AsyncRead`/`AsyncWrite` (Tokio) — base existante dans `ObfuscatedStream`.

**Corrections apportées à l'analyse initiale** :
1. Nonce ChaCha20-Poly1305 : jamais de nonce constant par clé (compteur ou nonce aléatoire par paquet).
2. Pas de conversion naïve Ed25519→X25519 : deux keypairs séparés.
3. Les relais doivent être des clients EMule-Rust — les 280 contacts KAD (eMule officiels) ignoreront tout opcode propriétaire. Problème de découverte des capacités = prérequis majeur non résolu.
4. Oignon UDP adapté aux requêtes KAD/chat (< MTU) ; transfert de blocs = tunnels TCP multi-sauts.
5. Arti/Tor écarté pour le transfert de fichiers (saturation, cellules 512 o, exit nodes bloquant le P2P).

---

## 5. Jalons v0.1.9

| Phase | Nom | Livrable | Crypto active |
|---|---|---|---|
| A0 | SUI Wire Specification | `docs/ed2k/SUI_WIRE_FORMAT.md` ✓ + fixtures + captures Tier 3 | Non |
| A1 | SUI Legacy Implementation | parseur `cryptkey.dat`, sign/verify réel, machine d'états, crédits | Oui, après A0 |
| A2 | SUI Interop Qualification | Tier 1/2/3 eMule 0.50a + aMule bidirectionnel | Validée |

**Verrou de sécurité (avant A1)** : remplacer `(48..=96).contains(&sig.len())` par un échec explicite — aucun `Verified`, aucun `credit_multiplier` sans vérification RSA réelle.

**Reste à faire en A0** : vecteurs hex Tier 3, fixture `cryptkey.dat` réel parsée octet par octet, documentation exacte du tag `m_bySupportSecIdent`, comportement `state > 2`, bornes `nSize` eMule.

**Spike technique préalable A1** : vérifier que la crate `rsa` accepte RSA-384 (génération et `from_components`) — sinon implémentation ciblée étroite dans `mule-crypto::legacy_sui` (opération historique uniquement, aucune API RSA générique).

---

## 6. Règles défensives

- Challenge CSPRNG (`OsRng`), consommé une seule fois, expiration courte (2-5 min), essais limités par pair/session.
- Aucune allocation dictée par une longueur réseau non bornée ; pas d'`unwrap` en chemin réseau ; fuzzing des décodeurs.
- Jamais de clé privée en SQLite, en logs, ou via RPC/MCP — empreintes tronquées uniquement.
- Écriture `cryptkey.dat` atomique (temp + `sync_all` + rename), permissions utilisateur restrictives.
- `mule-network-policy` consultée avant toute réponse — pas d'oracle ni d'amplificateur.
