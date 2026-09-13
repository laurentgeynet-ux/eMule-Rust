# Documentation des Vecteurs de Test et Provenance

Ce document recense la provenance, les paramètres et les hashs de référence des vecteurs de test utilisés pour valider `mule-format`.

---

## 1. Générateur Déterministe (`PatternReader`)

Afin d'éviter de stocker des fichiers volumineux (plusieurs dizaines de mégaoctets) dans l'historique Git, les tests exploitent le générateur cyclique standard :

* **Motif de base** : Séquence immuable d'octets `0x00, 0x01, 0x02, ..., 0xFF` (256 octets).
* **Répétition** : Cyclique jusqu'à atteindre la longueur cible exacte.
* **Garantie** : Déterministe, reproductible indépendamment sur toute plateforme, sans allocation excessive de mémoire vive ($O(1)$ RAM).

---

## 2. Table des Fixtures de Référence

| Identifiant | Longueur (octets) | Hash eD2K Attendu (hex) | Parts Attendues | Règle / Cas Limite Validé |
| :--- | :--- | :--- | :--- | :--- |
| `pattern_1_byte` | `1` | `47c61a0fa8738ba77308a8a600f88e4b` | 1 | Un seul octet non nul (`0x00`). |
| `pattern_9727999` | `9 727 999` | `4f2dc92b1fd24bde92dd01d85154b06c` | 1 | Taille limite juste sous une part eD2K complète ($n < 9\,728\,000$). |
| `pattern_9728000` | `9 728 000` | `d815a0409aaa0b4279dd86e580bb043f` | 2 | Exactement une part eD2K ($n = 9\,728\,000$). Append de `MD4(empty)`. |
| `pattern_9728001` | `9 728 001` | `695b81b1727666818988524bd7fc823c` | 2 | Une part complète + 1 octet résiduel. |
| `pattern_19456000` | `19 456 000` | `7fdd19736858b9f24c65da0e0e0adba3` | 3 | Exactement 2 parts eD2K ($2 \times 9\,728\,000$). Append de `MD4(empty)`. |

---

## 3. Règle du Hashset eMule sur Tailles Multiples

L'algorithme de hachage eMule présente une particularité historique documentée :
* Lorsqu'un fichier mesure **exactement un multiple** de `9 728 000` octets ($k \times 9\,728\,000$ avec $k \ge 1$), un bloc supplémentaire de taille zéro est inséré dans la liste des hashes de part avant le calcul du hash racine final.
* Le hash de ce bloc vide est le digest MD4 standard : `31d6cfe0d16ae931b73c59d7e0c089c0`.
* Pour un fichier d'exactement $9\,728\,000$ octets, la liste de parts comprend donc :
  1. `part_0`: MD4 des $9\,728\,000$ premiers octets.
  2. `part_1`: `31d6cfe0d16ae931b73c59d7e0c089c0` (bloc vide).
  Le hash racine est $\text{MD4}(\text{part}_0 \mathbin{\Vert} \text{part}_1)$.

---

## 4. Arbre Merkle AICH (180 KiB)

* **Taille d'un bloc de base** : `184 320` octets ($180\text{ KiB}$).
* **Part complète** : 53 feuilles ($52 \times 184\,320 + 143\,360 = 9\,728\,000$).
* **Vérification de Preuve (`AichProof`)** :
  * Validation structurelle : validation du nombre de feuilles (`leaf_count >= 1`), index (`leaf_index < leaf_count`), profondeur exacte et directions des frères (`SiblingPosition::Left` / `Right`) avant calcul cryptographique.
  * Validation cryptographique : remontée SHA-1 de la feuille vers la racine Merkle.
