# Notes de Compatibilité & Variantes Historiques eD2K

Ce document consigne les particularités de format, variantes d'encodage et divergences observées entre les implémentations historiques eMule, aMule et les spécifications d'origine eDonkey2000.

---

## 1. Identifiants Numériques vs Noms de Tags Textuels

Dans eDonkey2000 et les versions initiales d'eMule :
- Le nom d'un tag commence par un entier 16-bit little-endian indiquant sa longueur en octets.
- Si cette longueur vaut **1**, l'octet suivant n'est pas une chaîne ASCII d'un caractère, mais un **identifiant spécial standardisé** (`Special Tag ID`) :
  - `0x01` : Nom d'utilisateur / Nom de fichier (`CT_NAME` / `FT_FILENAME`)
  - `0x02` : Taille de fichier (`FT_FILESIZE`)
  - `0x03` : Type de fichier (`FT_FILETYPE`)
  - `0x11` : Version du client (`CT_VERSION`)
  - `0x5A` : Version eMule compatible (`CT_EMULE_VERSION`)
  - `0x5B` : Drapeaux de capacités étendues (`CT_EMULE_MISCOPTIONS1`)
  - `0x5C` : Drapeaux de capacités étendues 2 (`CT_EMULE_MISCOPTIONS2`)
  - `0xFB` : Port UDP eMule (`CT_EMULE_UDPOPTIONS`)
- Si la longueur est supérieure à 1, il s'agit d'une chaîne textuelle (ex: `"muleversion"` ou `"modversion"`).

### Évolution eMule 0.42+ (Extended Tags)
Dans les versions modernes d'eMule, un mécanisme d'optimisation compacte les tags en encodant des drapeaux binaires dans le bit de poids fort du type de tag (`tag_type & 0x80`), évitant d'écrire la longueur 1 pour les identifiants spéciaux.
Notre décodeur passif supporte à la fois la syntaxe classique à préfixe de longueur et capture les balises étendues.

---

## 2. Encodage des Chaînes de Caractères

Historiquement :
- Les clients eDonkey originaux utilisaient l'encodage ANSI/Windows-1252 ou ISO-8859-1 local de l'utilisateur.
- À partir d'eMule 0.47a et avec aMule 2.x, l'encodage standard par défaut pour les pseudonymes et les noms de fichiers est devenu **UTF-8**.
- **Gestion eMule-Rust** : Le décodeur tente une conversion stricte UTF-8. En cas de séquence invalide, il recourt à une conversion avec remplacement sans échec fatal (`String::from_utf8_lossy`), garantissant la résilience face à de vieux pairs ANSI.

---

## 3. HighID vs LowID dans le Client ID

Le champ `Client ID` (32-bit little-endian) transporte l'adresse réseau du pair :
- **HighID** : Le pair est directement joignable depuis l'extérieur sur son port TCP. Le `Client ID` est égal à l'adresse IPv4 publique du pair réordonnée en entier 32-bit little-endian. Une valeur $\ge 16\,777\,216$ ($2^{24}$) caractérise un HighID.
- **LowID** : Le pair est derrière un pare-feu ou un routeur NAT sans redirection de port. Le `Client ID` est un identifiant arbitraire attribué par le serveur eD2K ($< 16\,777\,216$). Deux pairs en LowID ne peuvent pas initier de connexion directe sans passer par un serveur intermédiaire.

---

## 4. IP et Port du Serveur en Fin de Handshake

Dans `OP_HELLO` et `OP_HELLOANSWER` :
- Après la liste des tags, eMule transmet l'adresse IPv4 (4 octets LE) et le port (2 octets LE) du serveur eD2K auquel le client est actuellement connecté.
- Si le client fonctionne sans être connecté à un serveur central (mode Kademlia pur ou hors-ligne), eMule et aMule écrivent `0.0.0.0:0` (6 octets nuls).
- Notre décodeur passif extrait ces champs optionnels sans présumer de leur signification réseau.
