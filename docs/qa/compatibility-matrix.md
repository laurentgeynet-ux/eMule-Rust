# Matrice de Compatibilité et Spécifications Protocolaire

Ce document formalise les choix d'interopérabilité et les garanties du moteur eMule moderne.

---

## 1. Algorithmes de Hachage et Intégrité

| Composant | Spécification eMule / aMule | Prise en charge `mule-format` | Notes / Cas Limites |
| :--- | :--- | :--- | :--- |
| **Taille d'une part eD2K** | Exactement `9 728 000` octets (9 500 KiB) | **Conforme** (`ED2K_PART_SIZE`) | Constante entière stricte. |
| **Hash eD2K ($0 < n < 9\,728\,000$)** | `MD4(file)` | **Conforme** | Un seul digest MD4 pour tout le fichier. |
| **Hash eD2K ($n = 9\,728\,000$)** | `MD4(MD4(part_0) \|\| MD4(empty))` | **Conforme** | Traité comme un hashset avec append de `MD4("")`. |
| **Hash eD2K ($n = k \times 9\,728\,000$)** | `MD4(part_0 \|\| ... \|\| MD4(empty))` | **Conforme** | Comportement officiel eMule préservé. |
| **Fichier vide ($n = 0$)** | Rejeté par eMule | **Rejeté** (`FormatError::EmptyInput`) | Considéré comme téléchargement invalide. |
| **AICH Bloc de base** | `184 320` octets (180 KiB) | **Conforme** (`AICH_BLOCK_SIZE`) | Arbre Merkle binaire sur base SHA-1. |
| **AICH Part eD2K complète** | 53 feuilles (52 de 180 KiB + 1 de 143 360 o) | **Conforme** | Découpage conforme à l'implémentation de référence. |
| **Preuve d'audit Merkle AICH** | Validation d'un bloc de 180 KiB corrompu | **Conforme** (`AichProof`) | Permet la récupération sélective d'un bloc corrompu. |

---

## 2. Formats d'échange et de stockage

| Format | Statut de lecture | Statut d'écriture | Rôle dans l'architecture |
| :--- | :--- | :--- | :--- |
| **Liens `ed2k://`** | Complet (strict + tags inconnus) | Canonical (`to_link_string`) | Saisie utilisateur, imports, partages. |
| **Fichiers `.part.met`** | Supporté (`0xe0`, `0xe1`, `0xe2`) | Prévu pour migration ultérieure | Import direct de téléchargements existants. |
| **Arbre AICH (`.aich`)** | Prévu | Prévu | Cache local des arbres Merkle. |
| **État interne `mule-core`** | Versionné (SQLite transactionnel) | Versionné (SQLite transactionnel) | Persistance moderne découplée des formats legacy. |

---

## 3. Découpage Architectural

```
┌─────────────────────────────────────────────────────────────┐
│                 apps/desktop (Flutter)                      │
│            Interface graphique utilisateur moderne          │
└──────────────────────────────┬──────────────────────────────┘
                               │ WebSocket / JSON-RPC local
┌──────────────────────────────▼──────────────────────────────┐
│                    crates/mule-daemon                       │
│       Processus d'arrière-plan, contrôle d'API locale       │
├─────────────────────────────────────────────────────────────┤
│                    crates/mule-core                         │
│   Gestionnaire de téléchargements, pièces, files, crédits   │
├─────────────────────────────────────────────────────────────┤
│                   crates/mule-format                        │
│    Hachage déterministe, décodage binaire, parsing liens    │
└─────────────────────────────────────────────────────────────┘
```
