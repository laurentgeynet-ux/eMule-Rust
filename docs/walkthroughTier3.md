# Walkthrough - Interopérabilité Tier 3 avec eMule 0.50a Officiel

Ce document certifie et consigne la résolution complète et le durcissement du handshake eD2K réel (Tier 3) entre notre implémentation Rust **Emule-Sion** et l'exécutable officiel de référence **eMule 0.50a** (`D:\eMule0.50a\emule.exe`, SHA-256 : `06cdf814387f627a4bd05a0c68211f715bfa952423e8e8a462e1f47c11a4d20e`), strictement confiné sur le loopback local `127.0.0.1`.

---

## 1. Diagnostics & Causes Racines Résolues

### Cause 1 : L'octet de longueur de hash `0x10` manquant dans `OP_HELLO`
- **Constat** : eMule fermait immédiatement la socket TCP après réception du premier paquet.
- **Explication** : Dans la spécification officielle eMule/eDonkey, le payload du paquet `OP_HELLO` (`0x01`) commence obligatoirement par un octet `0x10` (taille 16 en décimal) indiquant la longueur du User Hash qui suit immédiatement. À l'inverse, `OP_HELLOANSWER` (`0x4C`) ne possède pas ce préfixe.
- **Résolution** : Ajout du préfixe `0x10` dans `Ed2kEncoder::encode_hello` pour les profils eMule, portant le frame minimal à exactement 39 octets.

### Cause 2 : Mode répertoire portable (`UsePublicUserDirectories = 2`)
- **Constat** : Sur Windows moderne (Vista/7/10/11), eMule ignorait occasionnellement le `preferences.ini` local généré dans le bac à sable `%TEMP%\.tmp...` et basculait sur `%LOCALAPPDATA%\eMule\config`, liant un port aléatoire plutôt que le port éphémère du test.
- **Explication** : Le code source officiel d'eMule (`Preferences.cpp`) lit la valeur de registre `HKCU\Software\eMule\UsePublicUserDirectories` :
  - `0` : Mode multi-utilisateur (`%LOCALAPPDATA%`).
  - `1` : Mode utilisateur public.
  - `2` : Mode répertoire de l'exécutable (mode portable strict).
- **Résolution** : Le harnais applique automatiquement `reg add "HKCU\Software\eMule" /v UsePublicUserDirectories /t REG_DWORD /d 2 /f` avant le lancement, garantissant un confinement 100% hermétique dans le bac à sable temporaire.

### Cause 3 : Conflit d'instance unique sous Windows (Single-Instance Lock)
- **Constat** : Si un processus eMule orphelin existait en arrière-plan, toute nouvelle instance se terminait immédiatement avec le code de sortie 0.
- **Résolution** : Nettoyage proactif des processus orphelins avant lancement et dans la garde RAII `Tier3ProcessGuard`.

### Cause 4 : L'annonce concurrente `PR_EMULE` (`0xC5 0x01`) & Tags étendus
- **Constat** : eMule 0.50a émet simultanément `OP_EMULEINFO` (`0xC5 0x01`) et `OP_HELLOANSWER` (`0xE3 0x4C`).
- **Résolution** : Tolérance et conservation des paquets `PR_EMULE` dans l'état `AwaitingHelloAnswer`, décodage des tags canoniques eMule (`CTAG_EMULE_UDPPORTS = 0xF9`, `CTAG_EMULE_MISCOPTIONS1 = 0xFA`, `CTAG_EMULE_UDPOPTIONS = 0xFB`, `CTAG_EMULE_MISCOPTIONS2 = 0xFE`) et renseignement complet de `Ed2kPeerInfo`.

---

## 2. Structure Wire Validée du `OP_HELLO` (39 octets)

| Offset | Taille | Champ | Valeur sur le fil |
| :--- | :--- | :--- | :--- |
| **0** | 1 | Protocole eD2K | `0xE3` (`PR_ED2K`) |
| **1..4** | 4 | Longueur du payload (LE) | `0x22 0x00 0x00 0x00` (34 octets) |
| **5** | 1 | Opcode | `0x01` (`OP_HELLO`) |
| **6** | 1 | Préfixe taille User Hash | `0x10` (16 octets) |
| **7..22** | 16 | User Hash conforme eMule | `[.. 0x0E .. 0x6F ..]` (16 octets) |
| **23..26** | 4 | Client ID | `0x00 0x00 0x00 0x00` (Low ID / non assigné) |
| **27..28** | 2 | Port TCP local (LE) | Port éphémère alloué |
| **29..32** | 4 | Tag count | `0x00 0x00 0x00 0x00` (0 tags) |
| **33..36** | 4 | Server IP | `0x00 0x00 0x00 0x00` (aucun serveur) |
| **37..38** | 2 | Server Port | `0x00 0x00` (aucun port) |

**Total exact** : $1 + 4 + 34 = 39\text{ octets}$.

---

## 3. Traces Réelles d'Exécution Validée contre eMule 0.50a

Commande exécutée :
```powershell
powershell -ExecutionPolicy Bypass -File C:\Users\Lou\.gemini\antigravity\brain\b5461bae-da19-4e07-9c68-ad5cec420972\scratch\run_tier3.ps1
```

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.29s
     Running tests\tier3_loopback.rs (target\debug\deps\tier3_loopback-7797ba361086664e.exe)

running 1 test
test test_tier3_live_external_reference_loopback_handshake ... Target loopback addr: 127.0.0.1:57633
Reference binary work path: "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL"
Child PID: 18788
Attempt 1: child running = true
Attempt 1 failed: Timeout(500ms)
Attempt 2: child running = true
Attempt 2 failed: Timeout(500ms)
Attempt 3: child running = true
TCP connections for PID 18788:

LocalAddress LocalPort  State
------------ ---------  -----
0.0.0.0          57633 Listen

Connected on attempt 3!
Tier 3 live reference handshake successful! Peer info: Ed2kPeerInfo {
    user_hash: [180, 12, 178, 114, 6, 14, 174, 87, 189, 215, 84, 29, 202, 50, 111, 148],
    client_id: 0,
    tcp_port: 57633,
    name: Some("RefPeer050a"),
    version: Some(60),
    emule_version: None,
    misc_options1: Some(873677342),
    misc_options2: Some(11449),
    emule_udp_port: Some(57634),
    server_ip: 0,
    server_port: 0,
    tags: [
        Ed2kTag { name: Id(1), value: String("RefPeer050a") },
        Ed2kTag { name: Id(17), value: UInt32(60) },
        Ed2kTag { name: Id(249), value: UInt32(57634) },
        Ed2kTag { name: Id(250), value: UInt32(873677342) },
        Ed2kTag { name: Id(254), value: UInt32(11449) },
        Ed2kTag { name: Id(251), value: UInt32(51200) }
    ],
    unknown_tags: []
}
Files in work_path:
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\config"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\emule.exe"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\incoming"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\lang"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\preferences.ini"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\skins"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\temp"
Files in config_dir:
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\config\\cryptkey.dat"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\config\\downloads.txt"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\config\\preferences.ini"
  "C:\\Users\\Lou\\AppData\\Local\\Temp\\.tmphhTJHL\\config\\server.met"
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 5.75s
```

---

## 4. Tests Unitaires de Gel de Preuve

Trois tests unitaires garantissent désormais la non-régression de cette compatibilité au niveau `cargo test --workspace` sans nécessiter de binaire externe :

1. **`test_tier3_op_hello_wire_framing_emule050a_compliance`** :
   Valide au bit près la sérialisation et désérialisation du paquet `OP_HELLO` de 39 octets sous le profil `Emule050a`.
2. **`test_tier3_op_hello_answer_wire_framing_emule050a_compliance`** :
   Valide l'absence du préfixe `0x10` dans `OP_HELLOANSWER` et le décodage sans perte des tags étendus eMule (`CTAG_EMULE_UDPPORTS`, `CTAG_EMULE_MISCOPTIONS1`).
3. **`test_session_awaiting_hello_answer_accepts_pr_emule_extension_before_hello_answer`** :
   Valide que la machine à états de session tolère la réception de l'annonce `PR_EMULE 0xC5` (`OP_EMULEINFO`) concurrente avant `OP_HELLOANSWER` sans violer la machine à états et passe immédiatement à `Ready`.
