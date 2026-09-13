# Corpus de Fixtures eD2K Passives (Phase 5)

Ce répertoire contient le jeu de fixtures binaires contrôlé pour la validation des décodeurs en lecture seule et le durcissement adverse du protocole eD2K dans Emule-Sion.

---

## 1. Structure du Corpus

```text
fixtures/ed2k/
├── README.md
├── manifest.json
├── hello_v1_minimal.bin
├── hello_v1_tags.bin
├── hello_answer.bin
├── file_status.bin
└── malformed/
    ├── hello_truncated_header.bin
    ├── hello_truncated_tag.bin
    ├── oversized_string.bin
    ├── too_many_tags.bin
    └── unsupported_protocol_0xd4.bin
```

---

## 2. Classification des Fixtures

Conformément aux exigences architecturales de la Phase 5, chaque fixture est classée selon sa provenance :

1. **Construites à la main depuis une spécification** :
   - `hello_v1_minimal.bin` : Trame `OP_HELLO` standard minimale (hash 16 octets, Client ID, Port, 0 tags, 0.0.0.0:0).
   - `file_status.bin` : Trame `OP_FILESTATUS` avec 12 parts et masque binaire de 2 octets (`0xFF, 0x0F`).
2. **Dérivées des sources de référence eMule/aMule** :
   - `hello_v1_tags.bin` : Poignée de main issue des spécifications eMule/aMule (`CUpDownClient::SendHelloPacket`) contenant des balises spéciales (Nom, Version eMule) et une balise de texte standard (`prt`), plus adresse du serveur.
   - `hello_answer.bin` : Réponse `OP_HELLO_ANSWER` typique d'un pair eMule distant avec balise de nom.
3. **Entrées volontairement malformées (Hostile & Adversarial)** :
   - `malformed/hello_truncated_header.bin` : En-tête incomplet (< 5 octets).
   - `malformed/hello_truncated_tag.bin` : Tag déclarant 20 octets de chaîne mais tronqué après 4 octets.
   - `malformed/oversized_string.bin` : Chaîne de 1500 octets dépassant le plafond strict configuré (1024 octets).
   - `malformed/too_many_tags.bin` : Compteur de tags de 300 dépassant la borne maximale (256 tags).
   - `malformed/unsupported_protocol_0xd4.bin` : Trame utilisant le protocole compressé `0xD4`, devant être explicitement rejetée avant audit de décompression.

---

## 3. Contrat de Compatibilité via `manifest.json`

Le fichier `manifest.json` agit comme un contrat formel de compatibilité :
- Chaque fichier binaire est verrouillé par son empreinte cryptographique SHA-256 (`sha256`).
- Le résultat attendu (`expected`) et les détails contextuels (`expectedDetails`) spécifient le comportement exact imposé aux décodeurs : structure décodée pour les cas valides, ou type d'erreur, offset et contexte pour les cas malformés.
